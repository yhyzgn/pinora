//! Pinora 应用编排：生命周期、单实例与命令分发。

mod capture_fake;
mod capture_kde;
mod capture_select;
mod capture_xcap;
mod frame_cache;
mod hotkey;
mod image_sink;
mod kwin_place;
mod os_instance;
mod desktop_shell;
mod pin_window;
mod platform;
mod region_overlay;
mod region_workflow;
mod runtime;
mod single_instance;

pub use capture_fake::FakeCaptureProvider;
pub use capture_kde::KdeSpectacleCaptureProvider;
pub use capture_select::{
    fake_only, CaptureBackendKind, SelectedCaptureProvider,
};
pub use capture_xcap::XcapCaptureProvider;
pub use desktop_shell::run_desktop_shell;
pub use hotkey::{
    ensure_user_desktop_entry, FakeHotkeySource, GlobalHotkeyHub, GlobalHotkeyStatus, HotkeySource,
};
pub use image_sink::LocalImageSink;
pub use os_instance::OsSingleInstance;
pub use pin_window::{
    run_pin_session, scaled_window_size, PinSessionEnd, PinView,
};
pub use platform::{CapabilityProbe, FakeCapabilityProbe, RuntimeCapabilityProbe};
pub use region_overlay::run_region_selection;
pub use region_workflow::{capture_region_interactive, RegionCaptureResult};
pub use runtime::{AppRuntime, BootstrapOutcome, DispatchResult};
pub use single_instance::{
    InMemorySingleInstance, InstanceAcquisition, SingleInstance, SingleInstanceError,
};
