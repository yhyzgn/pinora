//! Pinora 桌面交互原语：贴图几何、Overlay 工具栏与预览缓存。
//!
//! 本 crate 只处理确定性的几何、交互布局和像素缓冲；它不创建窗口、事件循环、
//! 托盘、线程或平台资源。窗口和应用生命周期仍由上层编排。

pub mod kwin_place;
mod overlay_preview_cache;
mod overlay_toolbar;
mod pin_layout;
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
