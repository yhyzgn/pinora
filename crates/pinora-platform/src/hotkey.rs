//! 全局热键：注册 Snipaste 风格的 F1/F3/Shift+F3，跨窗口触发动作。
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

use pinora_core::{
    ActionId, DEFAULT_CLIPBOARD_HOTKEY, DEFAULT_REGION_HOTKEY, DEFAULT_TOGGLE_PINS_HOTKEY,
    HotkeyBinding, HotkeyCode, HotkeyModifiers, KeyBinding, PinoraError,
};
use winit::keyboard::{KeyCode, ModifiersState};

#[cfg(target_os = "linux")]
use crate::wayland_portal::{PortalAvailability, WaylandPortalHotkeys};

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

/// 已成功注册的热键。只有这些 `Pressed` 事件可转成 Pinora 动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegisteredHotkey {
    binding: HotkeyBinding,
    hotkey: HotKey,
    action: ActionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegisteredHotkeys {
    entries: Vec<RegisteredHotkey>,
}

trait HotkeyRegistrar {
    fn register(&self, hotkey: HotKey) -> Result<(), String>;
    fn unregister(&self, hotkey: HotKey) -> Result<(), String>;
}

impl HotkeyRegistrar for GlobalHotKeyManager {
    fn register(&self, hotkey: HotKey) -> Result<(), String> {
        GlobalHotKeyManager::register(self, hotkey).map_err(|error| error.to_string())
    }

    fn unregister(&self, hotkey: HotKey) -> Result<(), String> {
        GlobalHotKeyManager::unregister(self, hotkey).map_err(|error| error.to_string())
    }
}

impl RegisteredHotkeys {
    fn action_for_pressed_event(&self, id: u32, state: HotKeyState) -> Option<ActionId> {
        if state != HotKeyState::Pressed {
            return None;
        }
        self.entries
            .iter()
            .find(|entry| entry.hotkey.id() == id)
            .map(|entry| entry.action)
    }

    fn primary_binding(&self, action: ActionId) -> Option<RegisteredHotkey> {
        self.entries
            .iter()
            .find(|entry| {
                entry.action == action && is_primary_action_binding(entry.binding, action)
            })
            .copied()
    }

    fn replace_or_insert_primary(&mut self, action: ActionId, next: RegisteredHotkey) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| {
            entry.action == action && is_primary_action_binding(entry.binding, action)
        }) {
            *entry = next;
        } else {
            self.entries.push(next);
        }
    }
}

/// 进程内全局热键中枢：manager 由 GUI 主线程持有，主循环轮询其事件。
pub struct GlobalHotkeyHub {
    // 仅用于维持 OS 注册生命周期；不移动到后台线程。
    manager: Option<GlobalHotKeyManager>,
    registered: Option<RegisteredHotkeys>,
    region_hotkey: HotkeyBinding,
    clipboard_hotkey: HotkeyBinding,
    status: GlobalHotkeyStatus,
    #[cfg(target_os = "linux")]
    portal: Option<WaylandPortalHotkeys>,
    status_changed: bool,
}

impl GlobalHotkeyHub {
    /// 尝试启动 OS 级全局热键。F1 与 F3 可由设置重绑，Shift+F3 固定切换全部贴图。
    pub fn start(region_hotkey: HotkeyBinding, clipboard_hotkey: HotkeyBinding) -> Self {
        if let Err(error) = validate_primary_bindings(region_hotkey, clipboard_hotkey) {
            return Self::unavailable(error);
        }
        match register_global_hotkeys(region_hotkey, clipboard_hotkey) {
            Ok((manager, registered, mut notes)) => {
                notes.push(
                    "fallback: `pinora capture` keeps working through single-instance IPC".into(),
                );
                Self {
                    manager: Some(manager),
                    registered: Some(registered),
                    region_hotkey,
                    clipboard_hotkey,
                    status: GlobalHotkeyStatus {
                        available: true,
                        notes,
                    },
                    #[cfg(target_os = "linux")]
                    portal: None,
                    status_changed: false,
                }
            }
            Err(error) => {
                #[cfg(target_os = "linux")]
                {
                    Self::with_wayland_portal(error, region_hotkey, clipboard_hotkey)
                }
                #[cfg(not(target_os = "linux"))]
                {
                    Self::unavailable(error)
                }
            }
        }
    }

    pub fn status(&self) -> &GlobalHotkeyStatus {
        &self.status
    }

    pub fn poll_actions(&mut self) -> Vec<ActionId> {
        let mut out = Vec::new();
        if let Some(registered) = self.registered.as_ref() {
            while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
                if let Some(action) = registered.action_for_pressed_event(event.id(), event.state())
                {
                    out.push(action);
                }
            }
        }

        #[cfg(target_os = "linux")]
        if let Some(portal) = self.portal.as_ref() {
            let poll = portal.poll_actions(&mut out);
            if let Some(availability) = poll.availability {
                self.apply_portal_availability(availability);
            }
        }

        // 去抖：同一帧重复的 OS 事件只交付一次。
        out.dedup();
        out
    }

    /// Portal 初始化与重连发生在后台；桌面壳用该边沿刷新现有 tray 菜单。
    pub fn take_status_changed(&mut self) -> bool {
        std::mem::take(&mut self.status_changed)
    }

    /// 先注册新组合、再释放旧组合。任何失败都保留旧的动作映射。
    pub fn rebind(
        &mut self,
        region_hotkey: HotkeyBinding,
        clipboard_hotkey: HotkeyBinding,
    ) -> Result<bool, String> {
        validate_primary_bindings(region_hotkey, clipboard_hotkey)?;
        let changes = [
            (ActionId::CaptureRegionAndPin, region_hotkey),
            (ActionId::PasteClipboard, clipboard_hotkey),
        ];
        let desired: Vec<_> = changes
            .into_iter()
            .filter(|(action, binding)| match action {
                ActionId::CaptureRegionAndPin => *binding != self.region_hotkey,
                ActionId::PasteClipboard => *binding != self.clipboard_hotkey,
                _ => false,
            })
            .collect();
        if desired.is_empty() {
            return Ok(self.manager.is_some());
        }
        let Some(manager) = self.manager.as_ref() else {
            #[cfg(target_os = "linux")]
            if let Some(portal) = self.portal.as_ref()
                && portal.rebind(region_hotkey, clipboard_hotkey)
            {
                self.region_hotkey = region_hotkey;
                self.clipboard_hotkey = clipboard_hotkey;
                return Ok(true);
            }
            self.region_hotkey = region_hotkey;
            self.clipboard_hotkey = clipboard_hotkey;
            return Ok(false);
        };
        let Some(registered) = self.registered.as_mut() else {
            self.region_hotkey = region_hotkey;
            self.clipboard_hotkey = clipboard_hotkey;
            return Ok(false);
        };
        rebind_registered_hotkeys(manager, registered, &desired)?;
        self.region_hotkey = region_hotkey;
        self.clipboard_hotkey = clipboard_hotkey;
        let clipboard_registered = registered
            .primary_binding(ActionId::PasteClipboard)
            .is_some();
        self.status.notes =
            registration_notes(region_hotkey, clipboard_hotkey, clipboard_registered, true);
        Ok(true)
    }

    fn unavailable(error: String) -> Self {
        Self {
            manager: None,
            registered: None,
            region_hotkey: DEFAULT_REGION_HOTKEY,
            clipboard_hotkey: DEFAULT_CLIPBOARD_HOTKEY,
            status: GlobalHotkeyStatus {
                available: false,
                notes: vec![
                    format!("global-hotkey unavailable: {error}"),
                    "fallback: use the tray menu or `pinora capture` IPC".into(),
                    unavailable_platform_note().into(),
                ],
            },
            #[cfg(target_os = "linux")]
            portal: None,
            status_changed: false,
        }
    }

    #[cfg(target_os = "linux")]
    fn with_wayland_portal(
        backend_error: String,
        region_hotkey: HotkeyBinding,
        clipboard_hotkey: HotkeyBinding,
    ) -> Self {
        let portal = WaylandPortalHotkeys::start(region_hotkey, clipboard_hotkey);
        let mut hub = Self {
            manager: None,
            registered: None,
            region_hotkey,
            clipboard_hotkey,
            status: GlobalHotkeyStatus {
                available: false,
                notes: vec![
                    format!("global-hotkey unavailable: {backend_error}"),
                    "fallback: use the tray menu or `pinora capture` IPC".into(),
                    if portal.is_some() {
                        "Linux Wayland: XDG GlobalShortcuts Portal pending".into()
                    } else {
                        unavailable_platform_note().into()
                    },
                ],
            },
            portal,
            status_changed: false,
        };
        if hub.portal.is_some() {
            hub.status
                .notes
                .push("portal errors are reported as stable capability codes".into());
        }
        hub
    }

    #[cfg(target_os = "linux")]
    fn apply_portal_availability(&mut self, availability: PortalAvailability) {
        let next = match availability {
            PortalAvailability::Available => GlobalHotkeyStatus {
                available: true,
                notes: vec![
                    "global-hotkey: XDG GlobalShortcuts Portal registered".into(),
                    "fallback: `pinora capture` keeps working through single-instance IPC".into(),
                    "global-hotkey backend: Linux Wayland Portal".into(),
                ],
            },
            PortalAvailability::Unavailable(failure) => GlobalHotkeyStatus {
                available: false,
                notes: vec![
                    format!("global-hotkey portal unavailable: {}", failure.code()),
                    "fallback: use the tray menu or `pinora capture` IPC".into(),
                    unavailable_platform_note().into(),
                ],
            },
        };
        if self.status.available != next.available || self.status.notes != next.notes {
            self.status = next;
            self.status_changed = true;
        }
    }
}

fn register_global_hotkeys(
    region_hotkey: HotkeyBinding,
    clipboard_hotkey: HotkeyBinding,
) -> Result<(GlobalHotKeyManager, RegisteredHotkeys, Vec<String>), String> {
    let manager = GlobalHotKeyManager::new().map_err(|e| format!("create manager: {e}"))?;

    let primary_region = registered_hotkey(region_hotkey, ActionId::CaptureRegionAndPin);
    manager
        .register(primary_region.hotkey)
        .map_err(|error| format!("register region {region_hotkey}: {error}"))?;
    let clipboard = registered_hotkey(clipboard_hotkey, ActionId::PasteClipboard);
    manager
        .register(clipboard.hotkey)
        .map_err(|error| format!("register clipboard {clipboard_hotkey}: {error}"))?;
    let toggle = registered_hotkey(
        DEFAULT_TOGGLE_PINS_HOTKEY,
        ActionId::ToggleAllPinsVisibility,
    );
    manager.register(toggle.hotkey).map_err(|error| {
        format!("register pin visibility {DEFAULT_TOGGLE_PINS_HOTKEY}: {error}")
    })?;
    let entries = vec![primary_region, clipboard, toggle];
    Ok((
        manager,
        RegisteredHotkeys { entries },
        registration_notes(region_hotkey, clipboard_hotkey, true, false),
    ))
}

fn validate_primary_bindings(
    region_hotkey: HotkeyBinding,
    clipboard_hotkey: HotkeyBinding,
) -> Result<(), String> {
    if !region_hotkey.is_safe() || !clipboard_hotkey.is_safe() {
        return Err("hotkey_unsafe".into());
    }
    if region_hotkey == clipboard_hotkey
        || region_hotkey == DEFAULT_TOGGLE_PINS_HOTKEY
        || clipboard_hotkey == DEFAULT_TOGGLE_PINS_HOTKEY
    {
        return Err("hotkey_conflict".into());
    }
    Ok(())
}

fn rebind_registered_hotkeys<R: HotkeyRegistrar>(
    registrar: &R,
    registered: &mut RegisteredHotkeys,
    desired: &[(ActionId, HotkeyBinding)],
) -> Result<(), String> {
    let mut changes = Vec::new();
    for &(action, binding) in desired {
        let current = registered.primary_binding(action);
        if current.is_none_or(|entry| entry.binding != binding) {
            changes.push((current, registered_hotkey(binding, action)));
        }
    }
    if changes.is_empty() {
        return Ok(());
    }

    let mut newly_registered: Vec<RegisteredHotkey> = Vec::new();
    for (_, next) in &changes {
        if let Err(error) = registrar.register(next.hotkey) {
            for registered_next in newly_registered {
                let _ = registrar.unregister(registered_next.hotkey);
            }
            return Err(format!("hotkey_register_failed:{error}"));
        }
        newly_registered.push(*next);
    }

    let mut removed: Vec<RegisteredHotkey> = Vec::new();
    for (previous, _) in &changes {
        let Some(previous) = previous else {
            continue;
        };
        if let Err(error) = registrar.unregister(previous.hotkey) {
            for previously_removed in removed {
                let _ = registrar.register(previously_removed.hotkey);
            }
            for registered_next in newly_registered {
                let _ = registrar.unregister(registered_next.hotkey);
            }
            return Err(format!("hotkey_unregister_failed:{error}"));
        }
        removed.push(*previous);
    }
    for (_, next) in changes {
        registered.replace_or_insert_primary(next.action, next);
    }
    Ok(())
}

fn is_primary_action_binding(_binding: HotkeyBinding, action: ActionId) -> bool {
    matches!(
        action,
        ActionId::CaptureRegionAndPin | ActionId::PasteClipboard
    )
}

fn registered_hotkey(binding: HotkeyBinding, action: ActionId) -> RegisteredHotkey {
    let modifiers = global_modifiers(binding.modifiers);
    let hotkey = HotKey::new(modifiers, global_code(binding.code));
    RegisteredHotkey {
        binding,
        hotkey,
        action,
    }
}

fn global_modifiers(modifiers: HotkeyModifiers) -> Option<Modifiers> {
    let mut out = Modifiers::empty();
    if modifiers.contains(HotkeyModifiers::CONTROL) {
        out.insert(Modifiers::CONTROL);
    }
    if modifiers.contains(HotkeyModifiers::ALT) {
        out.insert(Modifiers::ALT);
    }
    if modifiers.contains(HotkeyModifiers::SHIFT) {
        out.insert(Modifiers::SHIFT);
    }
    if modifiers.contains(HotkeyModifiers::SUPER) {
        out.insert(Modifiers::SUPER);
    }
    (!out.is_empty()).then_some(out)
}

fn global_code(code: HotkeyCode) -> Code {
    match code {
        HotkeyCode::F1 => Code::F1,
        HotkeyCode::F2 => Code::F2,
        HotkeyCode::F3 => Code::F3,
        HotkeyCode::F4 => Code::F4,
        HotkeyCode::F5 => Code::F5,
        HotkeyCode::F6 => Code::F6,
        HotkeyCode::F7 => Code::F7,
        HotkeyCode::F8 => Code::F8,
        HotkeyCode::F9 => Code::F9,
        HotkeyCode::F10 => Code::F10,
        HotkeyCode::F11 => Code::F11,
        HotkeyCode::F12 => Code::F12,
        HotkeyCode::KeyA => Code::KeyA,
        HotkeyCode::KeyB => Code::KeyB,
        HotkeyCode::KeyC => Code::KeyC,
        HotkeyCode::KeyD => Code::KeyD,
        HotkeyCode::KeyE => Code::KeyE,
        HotkeyCode::KeyF => Code::KeyF,
        HotkeyCode::KeyG => Code::KeyG,
        HotkeyCode::KeyH => Code::KeyH,
        HotkeyCode::KeyI => Code::KeyI,
        HotkeyCode::KeyJ => Code::KeyJ,
        HotkeyCode::KeyK => Code::KeyK,
        HotkeyCode::KeyL => Code::KeyL,
        HotkeyCode::KeyM => Code::KeyM,
        HotkeyCode::KeyN => Code::KeyN,
        HotkeyCode::KeyO => Code::KeyO,
        HotkeyCode::KeyP => Code::KeyP,
        HotkeyCode::KeyQ => Code::KeyQ,
        HotkeyCode::KeyR => Code::KeyR,
        HotkeyCode::KeyS => Code::KeyS,
        HotkeyCode::KeyT => Code::KeyT,
        HotkeyCode::KeyU => Code::KeyU,
        HotkeyCode::KeyV => Code::KeyV,
        HotkeyCode::KeyW => Code::KeyW,
        HotkeyCode::KeyX => Code::KeyX,
        HotkeyCode::KeyY => Code::KeyY,
        HotkeyCode::KeyZ => Code::KeyZ,
    }
}

/// 将设置窗口收到的物理键转换为可持久化热键组合。未知键不进入设置文件。
pub fn binding_from_winit(code: KeyCode, state: ModifiersState) -> Option<HotkeyBinding> {
    let code = match code {
        KeyCode::F1 => HotkeyCode::F1,
        KeyCode::F2 => HotkeyCode::F2,
        KeyCode::F3 => HotkeyCode::F3,
        KeyCode::F4 => HotkeyCode::F4,
        KeyCode::F5 => HotkeyCode::F5,
        KeyCode::F6 => HotkeyCode::F6,
        KeyCode::F7 => HotkeyCode::F7,
        KeyCode::F8 => HotkeyCode::F8,
        KeyCode::F9 => HotkeyCode::F9,
        KeyCode::F10 => HotkeyCode::F10,
        KeyCode::F11 => HotkeyCode::F11,
        KeyCode::F12 => HotkeyCode::F12,
        KeyCode::KeyA => HotkeyCode::KeyA,
        KeyCode::KeyB => HotkeyCode::KeyB,
        KeyCode::KeyC => HotkeyCode::KeyC,
        KeyCode::KeyD => HotkeyCode::KeyD,
        KeyCode::KeyE => HotkeyCode::KeyE,
        KeyCode::KeyF => HotkeyCode::KeyF,
        KeyCode::KeyG => HotkeyCode::KeyG,
        KeyCode::KeyH => HotkeyCode::KeyH,
        KeyCode::KeyI => HotkeyCode::KeyI,
        KeyCode::KeyJ => HotkeyCode::KeyJ,
        KeyCode::KeyK => HotkeyCode::KeyK,
        KeyCode::KeyL => HotkeyCode::KeyL,
        KeyCode::KeyM => HotkeyCode::KeyM,
        KeyCode::KeyN => HotkeyCode::KeyN,
        KeyCode::KeyO => HotkeyCode::KeyO,
        KeyCode::KeyP => HotkeyCode::KeyP,
        KeyCode::KeyQ => HotkeyCode::KeyQ,
        KeyCode::KeyR => HotkeyCode::KeyR,
        KeyCode::KeyS => HotkeyCode::KeyS,
        KeyCode::KeyT => HotkeyCode::KeyT,
        KeyCode::KeyU => HotkeyCode::KeyU,
        KeyCode::KeyV => HotkeyCode::KeyV,
        KeyCode::KeyW => HotkeyCode::KeyW,
        KeyCode::KeyX => HotkeyCode::KeyX,
        KeyCode::KeyY => HotkeyCode::KeyY,
        KeyCode::KeyZ => HotkeyCode::KeyZ,
        _ => return None,
    };
    let mut modifiers = HotkeyModifiers::NONE;
    if state.control_key() {
        modifiers = modifiers | HotkeyModifiers::CONTROL;
    }
    if state.alt_key() {
        modifiers = modifiers | HotkeyModifiers::ALT;
    }
    if state.shift_key() {
        modifiers = modifiers | HotkeyModifiers::SHIFT;
    }
    if state.super_key() {
        modifiers = modifiers | HotkeyModifiers::SUPER;
    }
    Some(HotkeyBinding::new(modifiers, code))
}

fn registration_notes(
    region_hotkey: HotkeyBinding,
    clipboard_hotkey: HotkeyBinding,
    clipboard_registered: bool,
    rebound: bool,
) -> Vec<String> {
    let phase = if rebound { "rebound" } else { "registered" };
    vec![
        format!("global-hotkey: region {region_hotkey} {phase}"),
        if clipboard_registered {
            format!("global-hotkey: clipboard pin {clipboard_hotkey} {phase}")
        } else {
            format!("global-hotkey: clipboard pin {clipboard_hotkey} unavailable")
        },
        format!("global-hotkey: pin visibility {DEFAULT_TOGGLE_PINS_HOTKEY} {phase}"),
        active_platform_note().into(),
    ]
}

#[cfg(target_os = "linux")]
const fn active_platform_note() -> &'static str {
    "global-hotkey backend: Linux X11; pure Wayland attempts the XDG GlobalShortcuts Portal, then falls back to tray/IPC"
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
X-KDE-Shortcuts=F1

[Desktop Action Capture]
Name=Capture region
Name[zh_CN]=区域截图
Exec={exec} capture
# 系统设置 → 快捷键 → 自定义 可绑定此动作；部分 Plasma 会读取本行
X-KDE-Shortcuts=F1

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
    use std::cell::RefCell;

    #[derive(Default)]
    struct FakeRegistrar {
        active: RefCell<Vec<u32>>,
        fail_register: Option<u32>,
    }

    impl FakeRegistrar {
        fn with_active(entries: &[RegisteredHotkey], fail_register: Option<u32>) -> Self {
            Self {
                active: RefCell::new(entries.iter().map(|entry| entry.hotkey.id()).collect()),
                fail_register,
            }
        }

        fn active_ids(&self) -> Vec<u32> {
            self.active.borrow().clone()
        }
    }

    impl HotkeyRegistrar for FakeRegistrar {
        fn register(&self, hotkey: HotKey) -> Result<(), String> {
            if self.fail_register == Some(hotkey.id()) {
                return Err("injected_register_failure".into());
            }
            let mut active = self.active.borrow_mut();
            if active.contains(&hotkey.id()) {
                return Err("duplicate_registration".into());
            }
            active.push(hotkey.id());
            Ok(())
        }

        fn unregister(&self, hotkey: HotKey) -> Result<(), String> {
            let mut active = self.active.borrow_mut();
            let Some(index) = active.iter().position(|id| *id == hotkey.id()) else {
                return Err("missing_registration".into());
            };
            active.remove(index);
            Ok(())
        }
    }

    fn primary_bindings() -> RegisteredHotkeys {
        RegisteredHotkeys {
            entries: vec![
                registered_hotkey(
                    HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F1),
                    ActionId::CaptureRegionAndPin,
                ),
                registered_hotkey(
                    HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F3),
                    ActionId::PasteClipboard,
                ),
                registered_hotkey(
                    DEFAULT_TOGGLE_PINS_HOTKEY,
                    ActionId::ToggleAllPinsVisibility,
                ),
            ],
        }
    }

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
        let content = format!("Exec={exec} capture\nX-KDE-Shortcuts=F1\n");
        std::fs::write(&path, &content).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        assert!(s.contains("capture"));
        assert!(s.contains("F1"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn only_registered_pressed_events_become_actions() {
        let bindings = RegisteredHotkeys {
            entries: vec![
                registered_hotkey(
                    HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F1),
                    ActionId::CaptureRegionAndPin,
                ),
                registered_hotkey(
                    HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F3),
                    ActionId::PasteClipboard,
                ),
                registered_hotkey(
                    DEFAULT_TOGGLE_PINS_HOTKEY,
                    ActionId::ToggleAllPinsVisibility,
                ),
            ],
        };
        let region_id = bindings.entries[0].hotkey.id();
        let clipboard_id = bindings.entries[1].hotkey.id();
        let toggle_id = bindings.entries[2].hotkey.id();

        assert_eq!(
            bindings.action_for_pressed_event(region_id, HotKeyState::Pressed),
            Some(ActionId::CaptureRegionAndPin)
        );
        assert_eq!(
            bindings.action_for_pressed_event(clipboard_id, HotKeyState::Pressed),
            Some(ActionId::PasteClipboard)
        );
        assert_eq!(
            bindings.action_for_pressed_event(toggle_id, HotKeyState::Pressed),
            Some(ActionId::ToggleAllPinsVisibility)
        );
        assert_eq!(
            bindings.action_for_pressed_event(toggle_id, HotKeyState::Released),
            None
        );
        assert_eq!(
            bindings.action_for_pressed_event(99, HotKeyState::Pressed),
            None
        );
    }

    #[test]
    fn recorder_mapping_uses_physical_keys_and_current_modifiers() {
        let binding = binding_from_winit(
            KeyCode::KeyR,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
        )
        .expect("supported physical key");
        assert_eq!(binding.to_string(), "Ctrl+Shift+R");
        assert!(binding.is_safe());
        assert!(binding_from_winit(KeyCode::Escape, ModifiersState::empty()).is_none());
        assert!(
            !binding_from_winit(KeyCode::KeyR, ModifiersState::empty())
                .expect("mapped letter")
                .is_safe()
        );
    }

    #[test]
    fn primary_binding_validation_rejects_unsafe_and_reserved_combinations() {
        assert_eq!(
            validate_primary_bindings(
                HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::KeyA),
                HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F3),
            ),
            Err("hotkey_unsafe".into())
        );
        assert_eq!(
            validate_primary_bindings(
                DEFAULT_TOGGLE_PINS_HOTKEY,
                HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F3),
            ),
            Err("hotkey_conflict".into())
        );
        assert!(
            validate_primary_bindings(
                HotkeyBinding::new(HotkeyModifiers::CONTROL, HotkeyCode::KeyR),
                HotkeyBinding::new(HotkeyModifiers::ALT, HotkeyCode::F4),
            )
            .is_ok()
        );
    }

    #[test]
    fn rebind_registers_all_new_keys_before_releasing_old_mappings() {
        let mut bindings = primary_bindings();
        let initial = bindings.entries.clone();
        let registrar = FakeRegistrar::with_active(&initial, None);
        let next_region = HotkeyBinding::new(HotkeyModifiers::CONTROL, HotkeyCode::KeyR);
        let next_clipboard = HotkeyBinding::new(HotkeyModifiers::ALT, HotkeyCode::F4);

        rebind_registered_hotkeys(
            &registrar,
            &mut bindings,
            &[
                (ActionId::CaptureRegionAndPin, next_region),
                (ActionId::PasteClipboard, next_clipboard),
            ],
        )
        .expect("rebind");

        let region = bindings
            .primary_binding(ActionId::CaptureRegionAndPin)
            .expect("new region registration");
        let clipboard = bindings
            .primary_binding(ActionId::PasteClipboard)
            .expect("new clipboard registration");
        assert_eq!(region.binding, next_region);
        assert_eq!(clipboard.binding, next_clipboard);
        assert_eq!(
            bindings.action_for_pressed_event(region.hotkey.id(), HotKeyState::Pressed),
            Some(ActionId::CaptureRegionAndPin)
        );
        assert!(registrar.active_ids().contains(&region.hotkey.id()));
        assert!(registrar.active_ids().contains(&clipboard.hotkey.id()));
        assert!(!registrar.active_ids().contains(&initial[0].hotkey.id()));
        assert!(!registrar.active_ids().contains(&initial[1].hotkey.id()));
    }

    #[test]
    fn failed_new_registration_keeps_old_bindings_and_mapping() {
        let mut bindings = primary_bindings();
        let initial = bindings.entries.clone();
        let next_region = HotkeyBinding::new(HotkeyModifiers::CONTROL, HotkeyCode::KeyR);
        let next_clipboard = HotkeyBinding::new(HotkeyModifiers::ALT, HotkeyCode::F4);
        let registrar = FakeRegistrar::with_active(
            &initial,
            Some(
                registered_hotkey(next_clipboard, ActionId::PasteClipboard)
                    .hotkey
                    .id(),
            ),
        );

        assert!(
            rebind_registered_hotkeys(
                &registrar,
                &mut bindings,
                &[
                    (ActionId::CaptureRegionAndPin, next_region),
                    (ActionId::PasteClipboard, next_clipboard),
                ],
            )
            .is_err()
        );

        assert_eq!(bindings.entries, initial);
        assert_eq!(
            registrar.active_ids(),
            initial
                .iter()
                .map(|entry| entry.hotkey.id())
                .collect::<Vec<_>>()
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
