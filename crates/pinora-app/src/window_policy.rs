//! 辅助窗口的平台任务栏策略。
//!
//! Pinora 没有常驻主窗口；托盘是唯一后台入口。截图、贴图和面板窗口只在用户操作
//! 时短暂显示，并统一请求不出现在任务栏或 Dock 中。Wayland 的通用协议没有等价的
//! 任务栏提示，KDE Wayland 的补充处理由 `kwin_place` 在窗口映射后完成。

use winit::error::EventLoopError;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes};

/// 需要跳过任务栏的 Pinora 临时窗口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuxiliaryWindowKind {
    Overlay,
    Pin,
    Panel,
    DisplayHandle,
}

impl AuxiliaryWindowKind {
    const fn hides_from_taskbar(self) -> bool {
        match self {
            Self::Overlay | Self::Pin | Self::Panel | Self::DisplayHandle => true,
        }
    }

    const fn requires_post_map_policy(self) -> bool {
        match self {
            Self::Overlay | Self::Pin | Self::Panel => true,
            Self::DisplayHandle => false,
        }
    }
}

/// 创建辅助窗口，并在创建前统一应用平台任务栏/Dock 隔离策略。
///
/// 所有窗口必须由此入口创建；可见窗口在映射后还必须调用
/// [`apply_post_map_policy`] 以完成 KDE Wayland 的补充策略。
pub(crate) fn create_auxiliary_window(
    event_loop: &ActiveEventLoop,
    kind: AuxiliaryWindowKind,
    attributes: WindowAttributes,
) -> Result<Window, winit::error::OsError> {
    event_loop.create_window(auxiliary_window_attributes(kind, attributes))
}

/// 窗口映射后完成仅 KWin 可提供的任务栏/分页器隔离。
///
/// 调用方必须在窗口实际可见后调用。标准 Wayland 没有等价协议，因此只有检测到
/// KWin 时才执行脚本；失败只记录，不影响用户的当前操作。
pub(crate) fn apply_post_map_policy(kind: AuxiliaryWindowKind, title: &str) {
    if kind.requires_post_map_policy() && crate::kwin_place::kwin_available() {
        crate::kwin_place::mark_auxiliary_window_by_title(title, 50);
    }
}

fn auxiliary_window_attributes(
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
    use std::fs;
    use std::path::Path;

    #[test]
    fn every_auxiliary_window_kind_requests_taskbar_isolation() {
        for kind in [
            AuxiliaryWindowKind::Overlay,
            AuxiliaryWindowKind::Pin,
            AuxiliaryWindowKind::Panel,
            AuxiliaryWindowKind::DisplayHandle,
        ] {
            assert!(kind.hides_from_taskbar());
        }
    }

    #[test]
    fn every_visible_auxiliary_window_receives_post_map_policy() {
        for kind in [
            AuxiliaryWindowKind::Overlay,
            AuxiliaryWindowKind::Pin,
            AuxiliaryWindowKind::Panel,
        ] {
            assert!(kind.requires_post_map_policy());
        }
        assert!(!AuxiliaryWindowKind::DisplayHandle.requires_post_map_policy());
    }

    #[test]
    fn only_window_policy_may_construct_event_loops_or_windows() {
        let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let sources = fs::read_dir(&source_dir).expect("read application source directory");
        for entry in sources {
            let entry = entry.expect("read application source entry");
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
                continue;
            }
            let source = fs::read_to_string(&path).expect("read application source file");
            let file_name = path.file_name().and_then(|name| name.to_str());
            if source.contains("EventLoop::builder") || source.contains(".create_window(") {
                assert_eq!(file_name, Some("window_policy.rs"), "{path:?}");
            }
        }
    }
}
