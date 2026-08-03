//! Pinora 桌面交互与呈现边界：几何、Overlay 工具栏、窗口策略和受控视觉状态。
//!
//! 本 crate 只处理确定性的几何、布局、窗口属性和受控呈现状态；它不拥有应用
//! EventLoop、托盘句柄、业务线程或任务生命周期。窗口宿主和业务编排仍由上层负责。

pub mod kwin_place;
mod overlay_preview_cache;
mod overlay_toolbar;
pub mod panel_theme;
mod pin_layout;
pub mod tray_capabilities;
pub mod tray_feedback;
pub mod window_policy;

pub use overlay_preview_cache::OverlayPreviewCache;
pub use overlay_toolbar::{
    ToolbarAction, ToolbarButton, ToolbarPaintState, hit_test as toolbar_hit, layout_toolbar,
    paint_toolbar, toolbar_bounds,
};
pub use pin_layout::{
    PIN_MAX_SCALE, PIN_MIN_SCALE, PIN_PLACEMENT_GAP, PIN_RESIZE_GRIP, PinResizeHandle,
    PinResizeTarget, default_pin_position, fit_to_image_target, pin_resize_anchor_position,
    pin_resize_handle_at, pin_resize_target_from_drag, proportional_resize_target,
    scaled_window_size,
};
