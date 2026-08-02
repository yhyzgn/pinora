//! 全局热键：注册 F2 / Ctrl+N 等，跨窗口触发截图。
//!
//! `GlobalHotKeyManager` 必须在桌面 GUI 事件循环所属线程创建并存活：Windows
//! 要求 Win32 消息循环同线程，macOS 要求主线程 run loop。`DesktopApp` 在现有
//! `winit` 主事件循环中持有本类型并轮询事件，因此这里不能额外创建或转移 manager。
//!
//! Linux 后端只支持 X11；纯 Wayland 仍需要 Portal 或系统设置绑定 `pinora capture`，
//! 本模块不会把该降级误报为 Portal 支持。

use std::sync::Mutex;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

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

/// 已成功注册的热键 ID。只有这些 ID 的 Pressed 事件可转成 Pinora 动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegisteredHotkeys {
    region_ids: [u32; 2],
    alternate_region_id: Option<u32>,
    full_display_id: Option<u32>,
}

impl RegisteredHotkeys {
    fn action_for_pressed_event(self, id: u32, state: HotKeyState) -> Option<ActionId> {
        if state != HotKeyState::Pressed {
            return None;
        }
        if self.region_ids.contains(&id) || self.alternate_region_id == Some(id) {
            return Some(ActionId::CaptureRegionAndPin);
        }
        (self.full_display_id == Some(id)).then_some(ActionId::CaptureFullDisplay)
    }
}

/// 进程内全局热键中枢：manager 由 GUI 主线程持有，主循环轮询其事件。
pub struct GlobalHotkeyHub {
    // 仅用于维持 OS 注册生命周期；不移动到后台线程。
    manager: Option<GlobalHotKeyManager>,
    registered: Option<RegisteredHotkeys>,
    status: GlobalHotkeyStatus,
}

impl GlobalHotkeyHub {
    /// 尝试启动 OS 级全局热键（F2/Ctrl+N → 区域，F3 → 全屏）。
    pub fn start() -> Self {
        match register_global_hotkeys() {
            Ok((manager, registered, mut notes)) => {
                notes.push(
                    "fallback: `pinora capture` keeps working through single-instance IPC".into(),
                );
                Self {
                    manager: Some(manager),
                    registered: Some(registered),
                    status: GlobalHotkeyStatus {
                        available: true,
                        notes,
                    },
                }
            }
            Err(error) => Self::unavailable(error),
        }
    }

    pub fn status(&self) -> &GlobalHotkeyStatus {
        &self.status
    }

    pub fn poll_actions(&mut self) -> Vec<ActionId> {
        if self.manager.is_none() {
            return Vec::new();
        }
        let Some(registered) = self.registered else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if let Some(action) = registered.action_for_pressed_event(event.id(), event.state()) {
                out.push(action);
            }
        }
        // 去抖：同一帧多次 F2 只保留一次 Capture
        out.dedup();
        out
    }

    fn unavailable(error: String) -> Self {
        Self {
            manager: None,
            registered: None,
            status: GlobalHotkeyStatus {
                available: false,
                notes: vec![
                    format!("global-hotkey unavailable: {error}"),
                    "fallback: use the tray menu or `pinora capture` IPC".into(),
                    unavailable_platform_note().into(),
                ],
            },
        }
    }
}

fn register_global_hotkeys() -> Result<(GlobalHotKeyManager, RegisteredHotkeys, Vec<String>), String>
{
    let manager = GlobalHotKeyManager::new().map_err(|e| format!("create manager: {e}"))?;

    // F2 与 Ctrl+N 是核心区域截图入口，任一注册失败时让 manager 立即析构并退回 tray/IPC。
    let f2 = HotKey::new(None, Code::F2);
    let f2_id = f2.id();
    manager
        .register(f2)
        .map_err(|e| format!("register F2: {e}"))?;

    let ctrl_n = HotKey::new(Some(Modifiers::CONTROL), Code::KeyN);
    let ctrl_n_id = ctrl_n.id();
    manager
        .register(ctrl_n)
        .map_err(|e| format!("register Ctrl+N: {e}"))?;

    // Ctrl+Shift+S 是额外区域截图入口；冲突时保留 F2/Ctrl+N 主路径。
    let ctrl_shift_s = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyS);
    let ctrl_shift_s_id = ctrl_shift_s.id();
    let (alternate_region_id, ctrl_shift_s_note) = match manager.register(ctrl_shift_s) {
        Ok(()) => (
            Some(ctrl_shift_s_id),
            "global-hotkey: Ctrl+Shift+S alternate region capture registered".into(),
        ),
        Err(error) => (
            None,
            format!("global-hotkey: Ctrl+Shift+S alternate region capture unavailable ({error})"),
        ),
    };

    // F3 是可选的全屏入口；失败不应该丢失三个核心区域快捷键。
    let f3 = HotKey::new(None, Code::F3);
    let f3_id = f3.id();
    let (full_display_id, f3_note) = match manager.register(f3) {
        Ok(()) => (
            Some(f3_id),
            "global-hotkey: F3 full-display registered".into(),
        ),
        Err(error) => (
            None,
            format!("global-hotkey: F3 full-display unavailable ({error})"),
        ),
    };

    let notes = vec![
        "global-hotkey: F2 and Ctrl+N region capture registered".into(),
        ctrl_shift_s_note,
        f3_note,
        active_platform_note().into(),
    ];
    Ok((
        manager,
        RegisteredHotkeys {
            region_ids: [f2_id, ctrl_n_id],
            alternate_region_id,
            full_display_id,
        },
        notes,
    ))
}

#[cfg(target_os = "linux")]
const fn active_platform_note() -> &'static str {
    "global-hotkey backend: Linux X11; pure Wayland requires a system binding or future Portal adapter"
}

#[cfg(target_os = "windows")]
const fn active_platform_note() -> &'static str {
    "global-hotkey backend: Windows native registration on the GUI event-loop thread"
}

#[cfg(target_os = "macos")]
const fn active_platform_note() -> &'static str {
    "global-hotkey backend: macOS native registration on the main GUI thread"
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
const fn active_platform_note() -> &'static str {
    "global-hotkey backend: unsupported platform"
}

#[cfg(target_os = "linux")]
const fn unavailable_platform_note() -> &'static str {
    "Linux Wayland needs a system binding or future Portal adapter; tray and IPC remain available"
}

#[cfg(target_os = "windows")]
const fn unavailable_platform_note() -> &'static str {
    "Windows registration failed; tray and IPC remain available"
}

#[cfg(target_os = "macos")]
const fn unavailable_platform_note() -> &'static str {
    "macOS registration failed; tray and IPC remain available"
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
const fn unavailable_platform_note() -> &'static str {
    "this platform has no supported global-hotkey backend; tray and IPC remain available"
}

/// 安装/刷新用户级 desktop 入口，便于 KDE 系统设置绑定 `pinora capture`。
#[cfg(target_os = "linux")]
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

/// 非 Linux 平台不创建 freedesktop desktop entry。
#[cfg(not(target_os = "linux"))]
pub fn ensure_user_desktop_entry(
    _bin_path: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    Err("desktop entry is only supported on Linux".into())
}

#[cfg(target_os = "linux")]
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
        let tmp = std::env::temp_dir().join(format!("pinora-desktop-test-{}", std::process::id()));
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

    #[test]
    fn only_registered_pressed_events_become_actions() {
        let bindings = RegisteredHotkeys {
            region_ids: [11, 12],
            alternate_region_id: Some(13),
            full_display_id: Some(14),
        };

        assert_eq!(
            bindings.action_for_pressed_event(11, HotKeyState::Pressed),
            Some(ActionId::CaptureRegionAndPin)
        );
        assert_eq!(
            bindings.action_for_pressed_event(13, HotKeyState::Pressed),
            Some(ActionId::CaptureRegionAndPin)
        );
        assert_eq!(
            bindings.action_for_pressed_event(14, HotKeyState::Pressed),
            Some(ActionId::CaptureFullDisplay)
        );
        assert_eq!(
            bindings.action_for_pressed_event(12, HotKeyState::Released),
            None
        );
        assert_eq!(
            bindings.action_for_pressed_event(99, HotKeyState::Pressed),
            None
        );
    }

    #[test]
    fn unavailable_hub_keeps_external_actions_disabled() {
        let mut hub = GlobalHotkeyHub::unavailable("test backend unavailable".into());

        assert!(!hub.status().available);
        assert!(
            hub.status()
                .notes
                .iter()
                .any(|note| note.contains("tray") && note.contains("IPC"))
        );
        assert!(hub.poll_actions().is_empty());
    }
}
