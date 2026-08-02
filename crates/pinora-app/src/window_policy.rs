//! 辅助窗口的平台任务栏策略。
//!
//! Pinora 没有常驻主窗口；托盘是唯一后台入口。截图、贴图和面板窗口只在用户操作
//! 时短暂显示，并统一请求不出现在任务栏或 Dock 中。Wayland 的通用协议没有等价的
//! 任务栏提示，KDE Wayland 的补充处理由 `kwin_place` 在窗口映射后完成。

use winit::error::EventLoopError;
use winit::event_loop::EventLoop;
use winit::window::WindowAttributes;

/// 需要跳过任务栏的 Pinora 临时窗口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuxiliaryWindowKind {
    Overlay,
    Pin,
    Panel,
}

impl AuxiliaryWindowKind {
    const fn hides_from_taskbar(self) -> bool {
        match self {
            Self::Overlay | Self::Pin | Self::Panel => true,
        }
    }
}

/// 应用于所有可见辅助窗口的创建属性。
pub(crate) fn auxiliary_window_attributes(
    kind: AuxiliaryWindowKind,
    attributes: WindowAttributes,
) -> WindowAttributes {
    debug_assert!(kind.hides_from_taskbar());
    apply_platform_taskbar_policy(attributes)
}

#[cfg(target_os = "windows")]
fn apply_platform_taskbar_policy(attributes: WindowAttributes) -> WindowAttributes {
    use winit::platform::windows::WindowAttributesExtWindows;

    attributes.with_skip_taskbar(true)
}

#[cfg(target_os = "linux")]
fn apply_platform_taskbar_policy(attributes: WindowAttributes) -> WindowAttributes {
    use winit::platform::x11::{WindowAttributesExtX11, WindowType};

    attributes.with_x11_window_type(vec![WindowType::Utility])
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn apply_platform_taskbar_policy(attributes: WindowAttributes) -> WindowAttributes {
    attributes
}

/// 创建不进入 macOS Dock 的事件循环。
pub(crate) fn auxiliary_event_loop() -> Result<EventLoop<()>, EventLoopError> {
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        builder.with_activation_policy(ActivationPolicy::Accessory);
        builder.with_default_menu(false);
    }
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_auxiliary_window_kind_requests_taskbar_isolation() {
        for kind in [
            AuxiliaryWindowKind::Overlay,
            AuxiliaryWindowKind::Pin,
            AuxiliaryWindowKind::Panel,
        ] {
            assert!(kind.hides_from_taskbar());
        }
    }
}
