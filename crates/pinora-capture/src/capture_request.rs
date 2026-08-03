//! 捕获请求的纯值对象与 Overlay 初始选区策略。
//!
//! 本模块不调用截图后端，也不创建窗口；调用方以这些不可变请求意图选择真实后端，
//! 并保留 tray、失败恢复和 EventLoop 编排。

use pinora_core::{
    CaptureWindowInfo, DisplayId, DisplayInfo, ErrorCode, PinoraError, PixelRect, SelectionSession,
};

/// 新截图会话的交互模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Region,
    FullDisplay,
    AllDisplays,
    Window,
}

impl CaptureMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::FullDisplay => "full-display",
            Self::AllDisplays => "all-displays",
            Self::Window => "window",
        }
    }
}

/// 截图会话目标。窗口快照必须在实际捕获前由后端重新验证。
#[derive(Debug, Clone, PartialEq)]
pub enum CaptureTarget {
    DefaultLargest,
    Display(DisplayId),
    AllDisplays,
    Window(CaptureWindowInfo),
}

impl CaptureTarget {
    pub const fn log_label(&self) -> &'static str {
        match self {
            Self::DefaultLargest => "default-display",
            Self::Display(_) => "selected-display",
            Self::AllDisplays => "all-displays",
            Self::Window(_) => "selected-window",
        }
    }
}

/// Overlay 打开时的初始选区，不等同于图像如何取得。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayInitialSelection {
    Manual,
    FullImage,
}

pub const fn initial_selection_for_capture(capture_mode: CaptureMode) -> OverlayInitialSelection {
    match capture_mode {
        CaptureMode::Region => OverlayInitialSelection::Manual,
        CaptureMode::FullDisplay | CaptureMode::AllDisplays | CaptureMode::Window => {
            OverlayInitialSelection::FullImage
        }
    }
}

/// 解析目标显示器。显式目标消失时必须受控拒绝，不能回退到另一块屏幕。
pub fn resolve_capture_target(
    displays: &[DisplayInfo],
    target: &CaptureTarget,
) -> Result<DisplayInfo, PinoraError> {
    match target {
        CaptureTarget::DefaultLargest => displays
            .iter()
            .max_by_key(|display| display.bounds.size.area())
            .cloned()
            .ok_or_else(|| PinoraError::new(ErrorCode::NotFound, "no display for capture")),
        CaptureTarget::Display(display_id) => displays
            .iter()
            .find(|display| &display.id == display_id)
            .cloned()
            .ok_or_else(|| {
                PinoraError::new(
                    ErrorCode::NotFound,
                    format!("selected display is no longer available: {}", display_id.0),
                )
            }),
        CaptureTarget::AllDisplays | CaptureTarget::Window(_) => Err(PinoraError::new(
            ErrorCode::InvalidState,
            "non-display capture target cannot be resolved as a display",
        )),
    }
}

/// 将已冻结的初始选区策略应用到已经建立边界的会话。
pub fn apply_initial_selection(
    session: &mut SelectionSession,
    initial_selection: OverlayInitialSelection,
) -> Result<Option<PixelRect>, PinoraError> {
    match initial_selection {
        OverlayInitialSelection::Manual => Ok(None),
        OverlayInitialSelection::FullImage => session.select_all().map(Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{CaptureWindowId, PixelPoint};

    fn sample_displays() -> Vec<DisplayInfo> {
        vec![
            DisplayInfo {
                id: DisplayId::new("left"),
                name: "Left".into(),
                bounds: PixelRect::new(-1920, 0, 1920, 1080),
                scale: 1.0,
            },
            DisplayInfo {
                id: DisplayId::new("right"),
                name: "Right".into(),
                bounds: PixelRect::new(0, 0, 2560, 1440),
                scale: 1.25,
            },
        ]
    }

    fn sample_window() -> CaptureWindowInfo {
        CaptureWindowInfo {
            id: CaptureWindowId::from_raw(4),
            app_name: "Example".into(),
            title: "Private window".into(),
            bounds: PixelRect::new(1, 2, 3, 4),
            display: DisplayId::new("display"),
            scale: 1.0,
            is_minimized: false,
        }
    }

    #[test]
    fn modes_keep_stable_labels_and_initial_selection() {
        let cases = [
            (
                CaptureMode::Region,
                "region",
                OverlayInitialSelection::Manual,
            ),
            (
                CaptureMode::FullDisplay,
                "full-display",
                OverlayInitialSelection::FullImage,
            ),
            (
                CaptureMode::AllDisplays,
                "all-displays",
                OverlayInitialSelection::FullImage,
            ),
            (
                CaptureMode::Window,
                "window",
                OverlayInitialSelection::FullImage,
            ),
        ];

        for (mode, label, initial_selection) in cases {
            assert_eq!(mode.label(), label);
            assert_eq!(initial_selection_for_capture(mode), initial_selection);
        }
    }

    #[test]
    fn targets_keep_stable_log_labels() {
        let cases = [
            (CaptureTarget::DefaultLargest, "default-display"),
            (
                CaptureTarget::Display(DisplayId::new("display")),
                "selected-display",
            ),
            (CaptureTarget::AllDisplays, "all-displays"),
            (CaptureTarget::Window(sample_window()), "selected-window"),
        ];

        for (target, label) in cases {
            assert_eq!(target.log_label(), label);
        }
    }

    #[test]
    fn initial_selection_preserves_manual_and_full_image_behavior() {
        let bounds = PixelRect::new(-20, 15, 1920, 1080);
        let mut region = SelectionSession::new().with_bounds(bounds).with_min_edge(2);
        let mut full = SelectionSession::new().with_bounds(bounds).with_min_edge(2);

        assert_eq!(
            apply_initial_selection(
                &mut region,
                initial_selection_for_capture(CaptureMode::Region)
            )
            .unwrap(),
            None
        );
        assert_eq!(region.preview_rect(), None);
        assert_eq!(
            apply_initial_selection(
                &mut full,
                initial_selection_for_capture(CaptureMode::FullDisplay)
            )
            .unwrap(),
            Some(bounds)
        );
        assert_eq!(full.preview_rect(), Some(bounds));
        assert_eq!(full.anchor, Some(PixelPoint::new(-20, 15)));
    }

    #[test]
    fn default_target_selects_largest_display() {
        let selected = resolve_capture_target(&sample_displays(), &CaptureTarget::DefaultLargest)
            .expect("largest display");

        assert_eq!(selected.id, DisplayId::new("right"));
    }

    #[test]
    fn explicit_target_never_falls_back_to_another_display() {
        let displays = sample_displays();
        let selected =
            resolve_capture_target(&displays, &CaptureTarget::Display(DisplayId::new("left")))
                .expect("selected display");
        assert_eq!(selected.id, DisplayId::new("left"));

        let error = resolve_capture_target(
            &displays,
            &CaptureTarget::Display(DisplayId::new("unplugged")),
        )
        .expect_err("missing display must not fall back");
        assert_eq!(error.code, ErrorCode::NotFound);
    }

    #[test]
    fn non_display_targets_cannot_be_resolved_as_displays() {
        for target in [
            CaptureTarget::AllDisplays,
            CaptureTarget::Window(sample_window()),
        ] {
            let error = resolve_capture_target(&sample_displays(), &target)
                .expect_err("non-display target must be rejected");
            assert_eq!(error.code, ErrorCode::InvalidState);
        }
    }
}
