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

/// 创建保持隐藏的辅助窗口，并在创建前统一应用平台任务栏/Dock 隔离策略。
///
/// 所有窗口必须由此入口创建，并通过 [`show_auxiliary_window`] 映射为可见；这样
/// KWin 的映射后隔离不会依赖调用方记忆。
pub(crate) fn create_auxiliary_window(
    event_loop: &ActiveEventLoop,
    kind: AuxiliaryWindowKind,
    attributes: WindowAttributes,
) -> Result<Window, winit::error::OsError> {
    event_loop.create_window(auxiliary_window_attributes(kind, attributes))
}

/// 映射可见辅助窗口，并完成仅 KWin 可提供的任务栏/分页器隔离。
///
/// 标准 Wayland 没有等价协议，因此只有检测到 KWin 时才执行脚本；失败只记录，
/// 不影响用户当前操作。隐藏 display handle 不允许经此入口映射。
pub(crate) fn show_auxiliary_window(kind: AuxiliaryWindowKind, window: &Window, title: &str) {
    assert!(
        kind.requires_post_map_policy(),
        "display handle must remain hidden"
    );
    window.set_visible(true);
    apply_post_map_policy(kind, title);
}

/// 窗口映射后完成仅 KWin 可提供的任务栏/分页器隔离。
fn apply_post_map_policy(kind: AuxiliaryWindowKind, title: &str) {
    if kind.requires_post_map_policy() && crate::kwin_place::kwin_available() {
        crate::kwin_place::mark_auxiliary_window_by_title(title, 50);
    }
}

fn auxiliary_window_attributes(
    kind: AuxiliaryWindowKind,
    attributes: WindowAttributes,
) -> WindowAttributes {
    debug_assert!(kind.hides_from_taskbar());
    apply_platform_taskbar_policy(attributes).with_visible(false)
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
    use std::path::{Path, PathBuf};

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
    fn factory_forces_all_auxiliary_windows_to_start_hidden() {
        for kind in [
            AuxiliaryWindowKind::Overlay,
            AuxiliaryWindowKind::Pin,
            AuxiliaryWindowKind::Panel,
            AuxiliaryWindowKind::DisplayHandle,
        ] {
            let attrs =
                auxiliary_window_attributes(kind, Window::default_attributes().with_visible(true));
            assert!(!attrs.visible);
        }
    }

    #[test]
    fn only_window_policy_may_construct_or_show_windows() {
        let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let policy_path = source_dir.join("window_policy.rs");
        for path in rust_sources(&source_dir) {
            let source = fs::read_to_string(&path).expect("read application source file");
            if source.contains("EventLoop::builder")
                || source.contains(".create_window(")
                || source.contains(".with_visible(true)")
                || source.contains(".set_visible(true)")
            {
                assert_eq!(path, policy_path, "{path:?}");
            }
        }
    }

    fn rust_sources(dir: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        collect_rust_sources(dir, &mut paths);
        paths
    }

    fn collect_rust_sources(dir: &Path, paths: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("read application source directory") {
            let path = entry.expect("read application source entry").path();
            if path.is_dir() {
                collect_rust_sources(&path, paths);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                paths.push(path);
            }
        }
    }
}
