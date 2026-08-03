//! Pinora 系统托盘适配边界。
//!
//! 本 crate 只处理 tray-icon 菜单、事件和受限反馈；动作的业务编排、窗口资源和
//! 唯一 EventLoop 仍属于 `pinora-app`。

mod tray;

pub use tray::{AppTray, TrayAction, TrayPinListEntry};
