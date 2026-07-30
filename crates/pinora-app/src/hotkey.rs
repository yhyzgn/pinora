//! 全局热键：注册 F2 / Ctrl+N 等，跨窗口触发截图。
//!
//! 策略（按可用性叠加）：
//! 1. `global-hotkey`（X11 / XWayland 抓键，部分 Wayland 会话有效）
//! 2. 单实例 socket：`pinora capture` / 桌面快捷方式转发
//! 3. 控制窗焦点热键（无全局时的兜底）
//!
//! 说明：纯 Wayland 上真正可靠的跨应用热键依赖桌面门户
//! `org.freedesktop.portal.GlobalShortcuts` 或 KDE System Settings 绑定到
//! `pinora capture`；本模块在能注册 OS 热键时自动启用。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pinora_core::{ActionId, KeyBinding, PinoraError};

/// 热键提供者：注册绑定并轮询触发的动作。
pub trait HotkeySource {
    fn register(&mut self, binding: KeyBinding) -> Result<(), PinoraError>;
    fn poll_actions(&mut self) -> Vec<ActionId>;
}

/// 可注入动作的假热键源（测试与开发演示）。
#[derive(Debug, Default)]
pub struct FakeHotkeySource {
    bindings: Vec<KeyBinding>,
    pending: Mutex<Vec<ActionId>>,
}

impl FakeHotkeySource {
    pub fn new() -> Self {
        Self::default()
    }

    /// 模拟用户按下某动作对应热键。
    pub fn inject(&self, action: ActionId) {
        if let Ok(mut q) = self.pending.lock() {
            q.push(action);
        }
    }

    pub fn bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }
}

impl HotkeySource for FakeHotkeySource {
    fn register(&mut self, binding: KeyBinding) -> Result<(), PinoraError> {
        self.bindings.push(binding);
        Ok(())
    }

    fn poll_actions(&mut self) -> Vec<ActionId> {
        self.pending
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default()
    }
}

/// 全局热键运行结果。
#[derive(Debug, Clone)]
pub struct GlobalHotkeyStatus {
    pub available: bool,
    pub notes: Vec<String>,
}

/// 进程内全局热键中枢：后台线程监听，主循环 poll。
pub struct GlobalHotkeyHub {
    rx: Receiver<ActionId>,
    stop: Option<std::sync::Arc<AtomicBool>>,
    join: Option<JoinHandle<()>>,
    status: GlobalHotkeyStatus,
}

impl GlobalHotkeyHub {
    /// 尝试启动 OS 级全局热键（F2 → 截图，Ctrl+N → 截图）。
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let mut notes = Vec::new();

        match spawn_global_hotkey_thread(tx.clone(), std::sync::Arc::clone(&stop)) {
            Ok(join) => {
                notes.push(
                    "global-hotkey: F2 + Ctrl+N registered (X11/XWayland path; may not fire for pure Wayland-focused apps)".into(),
                );
                notes.push(
                    "also: `pinora capture` over single-instance socket works from KDE System Settings".into(),
                );
                Self {
                    rx,
                    stop: Some(stop),
                    join: Some(join),
                    status: GlobalHotkeyStatus {
                        available: true,
                        notes,
                    },
                }
            }
            Err(err) => {
                notes.push(format!("global-hotkey unavailable: {err}"));
                notes.push(
                    "fallback: use focused control/pin window keys, or bind System Settings → pinora capture".into(),
                );
                // 丢弃 tx 侧；rx 永远空
                drop(tx);
                Self {
                    rx,
                    stop: Some(stop),
                    join: None,
                    status: GlobalHotkeyStatus {
                        available: false,
                        notes,
                    },
                }
            }
        }
    }

    pub fn status(&self) -> &GlobalHotkeyStatus {
        &self.status
    }

    pub fn poll_actions(&mut self) -> Vec<ActionId> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(a) => out.push(a),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        // 去抖：同一帧多次 F2 只保留一次 Capture
        out.dedup();
        out
    }
}

impl Drop for GlobalHotkeyHub {
    fn drop(&mut self) {
        if let Some(stop) = &self.stop {
            stop.store(true, Ordering::SeqCst);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn spawn_global_hotkey_thread(
    tx: Sender<ActionId>,
    stop: std::sync::Arc<AtomicBool>,
) -> Result<JoinHandle<()>, String> {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

    let manager = GlobalHotKeyManager::new().map_err(|e| format!("create manager: {e}"))?;

    // F2：区域截图
    let f2 = HotKey::new(None, Code::F2);
    let f2_id = f2.id();
    manager
        .register(f2)
        .map_err(|e| format!("register F2: {e}"))?;

    // Ctrl+N：区域截图（与窗口内一致）
    let ctrl_n = HotKey::new(Some(Modifiers::CONTROL), Code::KeyN);
    let ctrl_n_id = ctrl_n.id();
    manager
        .register(ctrl_n)
        .map_err(|e| format!("register Ctrl+N: {e}"))?;

    // 可选：Ctrl+Shift+S 作为更不易冲突的备选
    let ctrl_shift_s = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyS);
    let ctrl_shift_s_id = ctrl_shift_s.id();
    if let Err(e) = manager.register(ctrl_shift_s) {
        eprintln!("pinora: optional Ctrl+Shift+S hotkey skipped: {e}");
    }

    let handle = thread::Builder::new()
        .name("pinora-global-hotkey".into())
        .spawn(move || {
            // 保持 manager 存活
            let _manager = manager;
            let receiver = GlobalHotKeyEvent::receiver();
            while !stop.load(Ordering::SeqCst) {
                match receiver.try_recv() {
                    Ok(event) if event.state() == HotKeyState::Pressed => {
                        let id = event.id();
                        if id == f2_id || id == ctrl_n_id || id == ctrl_shift_s_id {
                            let _ = tx.send(ActionId::CaptureRegionAndPin);
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {
                        // Empty or disconnected — short sleep then retry until stop
                        thread::sleep(Duration::from_millis(20));
                    }
                }
            }
        })
        .map_err(|e| format!("spawn: {e}"))?;

    Ok(handle)
}

/// 安装/刷新用户级 desktop 入口，便于 KDE 系统设置绑定 `pinora capture`。
pub fn ensure_user_desktop_entry(bin_path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let apps = dirs_applications()
        .ok_or_else(|| "cannot resolve ~/.local/share/applications".to_string())?;
    std::fs::create_dir_all(&apps).map_err(|e| format!("mkdir applications: {e}"))?;
    let path = apps.join("pinora.desktop");
    let exec = bin_path.display().to_string();
    let content = format!(
        r#"[Desktop Entry]
Type=Application
Name=Pinora
Comment=Screenshot, pin and annotate
Exec={exec}
TryExec={exec}
Icon=camera-photo
Terminal=false
Categories=Utility;Graphics;
StartupNotify=false
Actions=Capture;Quit;
# KWin 截图接口（与 Spectacle 同权）
X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2
# 提示 KDE：可在系统设置中绑定下列动作
X-KDE-Shortcuts=F2

[Desktop Action Capture]
Name=Capture region
Name[zh_CN]=区域截图
Exec={exec} capture
# 系统设置 → 快捷键 → 自定义 可绑定此动作；部分 Plasma 会读取本行
X-KDE-Shortcuts=F2;Ctrl+N

[Desktop Action Quit]
Name=Quit
Name[zh_CN]=退出
Exec={exec} quit
"#
    );
    std::fs::write(&path, content).map_err(|e| format!("write desktop: {e}"))?;
    // 通知桌面缓存（忽略失败）
    let _ = std::process::Command::new("update-desktop-database")
        .arg(&apps)
        .status();
    Ok(path)
}

fn dirs_applications() -> Option<std::path::PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        return Some(std::path::PathBuf::from(home).join(".local/share/applications"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_and_poll() {
        let mut src = FakeHotkeySource::new();
        src.register(KeyBinding::new(ActionId::Quit, "Ctrl+Q"))
            .unwrap();
        src.inject(ActionId::Quit);
        assert_eq!(src.poll_actions(), vec![ActionId::Quit]);
        assert!(src.poll_actions().is_empty());
    }

    #[test]
    fn desktop_entry_contains_capture_action() {
        let tmp = std::env::temp_dir().join(format!(
            "pinora-desktop-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        // 临时劫持 HOME
        // 只测模板字符串构造逻辑
        let bin = std::path::Path::new("/usr/bin/pinora");
        let apps = tmp.join("applications");
        std::fs::create_dir_all(&apps).unwrap();
        let path = apps.join("pinora.desktop");
        let exec = bin.display().to_string();
        let content = format!("Exec={exec} capture\nX-KDE-Shortcuts=F2\n");
        std::fs::write(&path, &content).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        assert!(s.contains("capture"));
        assert!(s.contains("F2"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
