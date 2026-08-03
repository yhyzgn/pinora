//! 捕获会话的纯状态和值对象。
//!
//! 本模块只描述捕获模式、延时恢复范围和 Overlay 目标。实际捕获、线程、窗口、
//! EventLoop、托盘和恢复副作用继续由应用桌面壳编排。

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use pinora_core::{
    CaptureImage, CaptureWindowInfo, DisplayId, ErrorCode, PinId, PixelPoint, PixelRect, PixelSize,
};

use crate::{CapturePreview, CaptureTarget, OverlayInitialSelection};

/// 捕获会话的瞬态模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureSessionMode {
    /// 下一帧启动：后台截屏（无全屏遮罩，避免截到自己）。
    StartCapture,
    /// 正在后台截屏，显示小加载窗。
    LoadingCapture,
    /// tray 发起的无窗口倒计时；到期后只能走冷捕获。
    DelayedCapture,
    /// 空闲：仅贴图窗口。
    Idle,
}

/// `LoadingState` 失败时必须采用的恢复路径。延时会话优先，因为它拥有需要恢复的
/// 贴图可见性快照；正常和窗口截图不应以失败退出 tray 主循环。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureFailureScope {
    Standard,
    Window,
    Delayed,
}

pub fn capture_failure_scope(target: &CaptureTarget, delayed_active: bool) -> CaptureFailureScope {
    if delayed_active {
        CaptureFailureScope::Delayed
    } else if matches!(target, CaptureTarget::Window(_)) {
        CaptureFailureScope::Window
    } else {
        CaptureFailureScope::Standard
    }
}

/// Overlay 的窗口呈现方式。历史编辑不能假装当前桌面仍是原始全屏捕获。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPresentation {
    ScreenCapture,
    VirtualDesktop,
    WindowCapture,
    HistoryEditor,
    PinEditor,
}

/// 截屏中：后台抓当前屏（无全屏遮罩，避免截到自己）；完成后立刻开真实 overlay。
pub struct LoadingState {
    // 后台捕获错误只跨线程传递稳定错误码，避免平台后端文本泄露窗口身份或标题。
    pub preview_rx: Receiver<Result<CapturePreview, ErrorCode>>,
    pub target: OverlayTarget,
}

/// 延时区域截图的清理所有者。
///
/// 快照只保存倒计时开始时由 Pinora 确认可见的贴图领域 ID；恢复时已经关闭的贴图
/// 会被忽略，因此不会复活用户已经关闭的贴图。
pub struct DelayedCapture {
    deadline: Instant,
    hidden_pin_ids: Vec<PinId>,
}

impl DelayedCapture {
    pub fn new(delay: Duration, hidden_pin_ids: Vec<PinId>) -> Self {
        Self {
            deadline: Instant::now() + delay,
            hidden_pin_ids,
        }
    }

    pub fn is_due(&self, now: Instant) -> bool {
        now >= self.deadline
    }

    pub fn hidden_pin_ids(&self) -> &[PinId] {
        &self.hidden_pin_ids
    }
}

/// 打开 Overlay 所需的捕获来源与初始交互意图。
pub struct OverlayTarget {
    pub display_id: DisplayId,
    pub display_origin: PixelPoint,
    pub image_width: u32,
    pub image_height: u32,
    pub initial_selection: OverlayInitialSelection,
    pub presentation: OverlayPresentation,
    pub min_selection_edge: u32,
    pub edit_pin_id: Option<PinId>,
}

impl OverlayTarget {
    pub fn update_image_dimensions(&mut self, width: u32, height: u32) {
        self.image_width = width;
        self.image_height = height;
    }
}

pub fn screen_capture_overlay_target(
    display_id: DisplayId,
    display_origin: PixelPoint,
    image_width: u32,
    image_height: u32,
    initial_selection: OverlayInitialSelection,
) -> OverlayTarget {
    full_image_overlay_target(
        display_id,
        display_origin,
        PixelSize::new(image_width, image_height),
        initial_selection,
        OverlayPresentation::ScreenCapture,
        2,
        None,
    )
}

pub fn virtual_desktop_overlay_target(
    workspace: PixelRect,
    initial_selection: OverlayInitialSelection,
) -> OverlayTarget {
    full_image_overlay_target(
        DisplayId::virtual_desktop(),
        workspace.origin,
        workspace.size,
        initial_selection,
        OverlayPresentation::VirtualDesktop,
        2,
        None,
    )
}

pub fn history_edit_target(image: &CaptureImage) -> OverlayTarget {
    // 输出保持历史图像原始来源坐标；窗口位置不假定旧显示器仍存在。
    full_image_overlay_target(
        image.metadata.display.clone(),
        image.source_rect.origin,
        image.pixels.size,
        OverlayInitialSelection::FullImage,
        OverlayPresentation::HistoryEditor,
        1,
        None,
    )
}

pub fn window_capture_overlay_target(window: &CaptureWindowInfo) -> OverlayTarget {
    full_image_overlay_target(
        window.display.clone(),
        window.bounds.origin,
        window.bounds.size,
        OverlayInitialSelection::FullImage,
        OverlayPresentation::WindowCapture,
        1,
        None,
    )
}

pub fn pin_edit_target(image: &CaptureImage, pin_id: PinId) -> OverlayTarget {
    full_image_overlay_target(
        image.metadata.display.clone(),
        image.source_rect.origin,
        image.pixels.size,
        OverlayInitialSelection::FullImage,
        OverlayPresentation::PinEditor,
        1,
        Some(pin_id),
    )
}

pub fn snapshot_visible_ids<T: Copy>(items: impl IntoIterator<Item = (T, bool)>) -> Vec<T> {
    items
        .into_iter()
        .filter_map(|(id, visible)| visible.then_some(id))
        .collect()
}

fn full_image_overlay_target(
    display_id: DisplayId,
    display_origin: PixelPoint,
    image_size: PixelSize,
    initial_selection: OverlayInitialSelection,
    presentation: OverlayPresentation,
    min_selection_edge: u32,
    edit_pin_id: Option<PinId>,
) -> OverlayTarget {
    OverlayTarget {
        display_id,
        display_origin,
        image_width: image_size.width,
        image_height: image_size.height,
        initial_selection,
        presentation,
        min_selection_edge,
        edit_pin_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{CaptureMetadata, CaptureWindowId, ImageId, RgbaBuffer};

    #[test]
    fn delayed_failure_recovery_precedes_window_and_standard_capture_scopes() {
        let window = CaptureTarget::Window(CaptureWindowInfo {
            id: CaptureWindowId::from_raw(4),
            app_name: "Example".into(),
            title: "Private window".into(),
            bounds: PixelRect::new(1, 2, 3, 4),
            display: DisplayId::new("display"),
            scale: 1.0,
            is_minimized: false,
        });

        assert_eq!(
            capture_failure_scope(&CaptureTarget::DefaultLargest, false),
            CaptureFailureScope::Standard
        );
        assert_eq!(
            capture_failure_scope(&window, false),
            CaptureFailureScope::Window
        );
        assert_eq!(
            capture_failure_scope(&window, true),
            CaptureFailureScope::Delayed
        );
    }

    #[test]
    fn standard_and_virtual_desktop_targets_preserve_the_capture_mapping() {
        let standard = screen_capture_overlay_target(
            DisplayId::new("display"),
            PixelPoint::new(-1_920, 0),
            1_920,
            1_080,
            OverlayInitialSelection::Manual,
        );
        let virtual_desktop = virtual_desktop_overlay_target(
            PixelRect::new(-1_920, -100, 3_840, 1_180),
            OverlayInitialSelection::FullImage,
        );

        assert_eq!(standard.display_id, DisplayId::new("display"));
        assert_eq!(standard.display_origin, PixelPoint::new(-1_920, 0));
        assert_eq!(standard.image_width, 1_920);
        assert_eq!(standard.image_height, 1_080);
        assert_eq!(standard.initial_selection, OverlayInitialSelection::Manual);
        assert_eq!(standard.presentation, OverlayPresentation::ScreenCapture);
        assert_eq!(standard.min_selection_edge, 2);
        assert_eq!(standard.edit_pin_id, None);

        assert_eq!(virtual_desktop.display_id, DisplayId::virtual_desktop());
        assert_eq!(
            virtual_desktop.display_origin,
            PixelPoint::new(-1_920, -100)
        );
        assert_eq!(virtual_desktop.image_width, 3_840);
        assert_eq!(virtual_desktop.image_height, 1_180);
        assert_eq!(
            virtual_desktop.initial_selection,
            OverlayInitialSelection::FullImage
        );
        assert_eq!(
            virtual_desktop.presentation,
            OverlayPresentation::VirtualDesktop
        );
        assert_eq!(virtual_desktop.min_selection_edge, 2);
        assert_eq!(virtual_desktop.edit_pin_id, None);
    }

    #[test]
    fn window_capture_opens_a_full_image_editor_without_a_display_capture_target() {
        let window = CaptureWindowInfo {
            id: CaptureWindowId::from_raw(3),
            app_name: "Example".into(),
            title: "Private window".into(),
            bounds: PixelRect::new(40, 50, 800, 600),
            display: DisplayId::new("window-display"),
            scale: 1.25,
            is_minimized: false,
        };

        let target = window_capture_overlay_target(&window);

        assert_eq!(target.display_id, window.display);
        assert_eq!(target.display_origin, window.bounds.origin);
        assert_eq!(target.image_width, 800);
        assert_eq!(target.image_height, 600);
        assert_eq!(target.initial_selection, OverlayInitialSelection::FullImage);
        assert_eq!(target.presentation, OverlayPresentation::WindowCapture);
        assert_eq!(target.min_selection_edge, 1);
        assert_eq!(target.edit_pin_id, None);
    }

    #[test]
    fn pin_and_history_edit_targets_keep_the_original_image_coordinates() {
        let pin_id = PinId::from_raw(37);
        let image = CaptureImage::new(
            ImageId::from_raw(38),
            RgbaBuffer::solid(PixelSize::new(800, 600), [1, 2, 3, 255]),
            PixelRect::new(240, -30, 800, 600),
            CaptureMetadata::new(DisplayId::new("source-display"), 1.25, 77),
        )
        .expect("image");
        let pin = pin_edit_target(&image, pin_id);
        let history = history_edit_target(&image);

        for target in [&pin, &history] {
            assert_eq!(target.display_id, image.metadata.display);
            assert_eq!(target.display_origin, image.source_rect.origin);
            assert_eq!(target.image_width, 800);
            assert_eq!(target.image_height, 600);
            assert_eq!(target.initial_selection, OverlayInitialSelection::FullImage);
            assert_eq!(target.min_selection_edge, 1);
        }
        assert_eq!(pin.presentation, OverlayPresentation::PinEditor);
        assert_eq!(pin.edit_pin_id, Some(pin_id));
        assert_eq!(history.presentation, OverlayPresentation::HistoryEditor);
        assert_eq!(history.edit_pin_id, None);
    }

    #[test]
    fn delayed_capture_keeps_only_visible_pin_ids_and_is_not_due_early() {
        let snapshot = snapshot_visible_ids([
            (PinId::from_raw(11), true),
            (PinId::from_raw(12), false),
            (PinId::from_raw(13), true),
        ]);
        let delayed = DelayedCapture::new(Duration::from_secs(60), snapshot.clone());

        assert_eq!(delayed.hidden_pin_ids(), snapshot);
        assert!(!delayed.is_due(Instant::now()));
    }

    #[test]
    fn capture_completion_can_replace_the_initial_target_dimensions() {
        let mut target = screen_capture_overlay_target(
            DisplayId::new("display"),
            PixelPoint::new(0, 0),
            1,
            1,
            OverlayInitialSelection::Manual,
        );

        target.update_image_dimensions(800, 600);

        assert_eq!((target.image_width, target.image_height), (800, 600));
    }
}
