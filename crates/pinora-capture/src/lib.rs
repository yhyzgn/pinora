//! Pinora 捕获能力：真实后端选择、显式测试 fake 和预截帧缓存。
//!
//! 本 crate 不创建窗口、不拥有 UI 状态，也不决定失败后的用户交互；所有平台结果
//! 通过 `pinora-core::CaptureProvider` 契约返回，供应用层编排。

mod capture_fake;
mod capture_kde;
mod capture_preview;
mod capture_request;
mod capture_select;
mod capture_xcap;
mod frame_cache;

pub use capture_fake::FakeCaptureProvider;
pub use capture_kde::KdeSpectacleCaptureProvider;
pub use capture_preview::{CapturePreview, rgba_to_xrgb, rgba_to_xrgb_and_dim};
pub use capture_request::{
    CaptureMode, CaptureTarget, OverlayInitialSelection, apply_initial_selection,
    initial_selection_for_capture, resolve_capture_target,
};
pub use capture_select::{CaptureBackendKind, SelectedCaptureProvider, fake_only};
pub use capture_xcap::XcapCaptureProvider;
pub use frame_cache::{CachedFrame, FrameCache};
