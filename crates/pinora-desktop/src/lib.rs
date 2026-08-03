//! Pinora 桌面交互与呈现边界：几何、Overlay 工具栏、窗口策略和受控视觉状态。
//!
//! 本 crate 只处理确定性的几何、布局、窗口属性和受控呈现状态；它不拥有应用
//! EventLoop、托盘句柄、业务线程或任务生命周期。窗口宿主和业务编排仍由上层负责。

pub mod diagnostics_panel;
pub mod history_browser;
pub mod kwin_place;
mod overlay_geometry;
mod overlay_preview_cache;
pub mod overlay_selection_readout;
mod overlay_toolbar;
pub mod panel_theme;
pub mod pin_context_menu;
mod pin_layout;
pub mod settings_panel;
pub mod tray_capabilities;
pub mod tray_feedback;
pub mod window_policy;
mod xrgb;

pub use overlay_geometry::{
    SELECTION_HANDLE_HIT_RADIUS, buffer_rect_to_source, selection_handle_at,
    selection_resize_allowed, selection_to_annotation_local, window_point_to_image,
    window_rect_from_points, window_selection_to_image,
};
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
pub use xrgb::{
    PinRenderCache, XRGB_SELECTION_HANDLE_RENDER_RADIUS, blit_xrgb_rect, build_pin_render_cache,
    draw_xrgb_border, draw_xrgb_outline, draw_xrgb_rect_border, draw_xrgb_selection_handles,
    scale_xrgb_nearest, xrgb_pixel_count,
};
