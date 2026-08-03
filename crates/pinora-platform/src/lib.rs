//! Pinora 系统集成能力。
//!
//! 本 crate 只承载操作系统交互与生命周期端口：用户级启动项、单实例/IPC、全局
//! 热键和 Wayland Portal。它不得依赖 `pinora-app` 或任何桌面业务状态。

mod hotkey;
mod os_instance;
mod single_instance;
mod start_on_login;
#[cfg(target_os = "linux")]
mod wayland_portal;

pub use hotkey::{
    FakeHotkeySource, GlobalHotkeyHub, GlobalHotkeyStatus, HotkeySource, binding_from_winit,
    ensure_user_desktop_entry,
};
pub use os_instance::{OsSingleInstance, forward_ipc_frame};
pub use single_instance::{
    InMemorySingleInstance, InstanceAcquisition, SingleInstance, SingleInstanceError,
};
pub use start_on_login::{StartOnLoginError, set_enabled as set_start_on_login_enabled};
