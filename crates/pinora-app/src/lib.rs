//! Pinora 应用编排：生命周期、单实例与命令分发。

mod capture_fake;
mod capture_kde;
mod capture_select;
mod capture_xcap;
mod desktop_shell;
mod export_job;
mod frame_cache;
mod hotkey;
mod image_sink;
mod job_supervisor;
mod kwin_place;
mod ocr;
mod ocr_job;
mod os_instance;
mod overlay_toolbar;
mod pin_window;
mod platform;
mod region_overlay;
mod region_workflow;
mod runtime;
mod single_instance;
mod tray;
mod worker_lifecycle;

pub use capture_fake::FakeCaptureProvider;
pub use capture_kde::KdeSpectacleCaptureProvider;
pub use capture_select::{CaptureBackendKind, SelectedCaptureProvider, fake_only};
pub use capture_xcap::XcapCaptureProvider;
pub use desktop_shell::run_desktop_shell;
pub use export_job::{
    ExportJobCompletion, ExportJobInput, ExportJobService, ExportRunner, LocalExportRunner,
};
pub use hotkey::{
    FakeHotkeySource, GlobalHotkeyHub, GlobalHotkeyStatus, HotkeySource, ensure_user_desktop_entry,
};
pub use image_sink::{
    LocalImageSink, copy_text_to_system_clipboard, detect_system_clipboard_backend,
};
pub use job_supervisor::{
    AcceptedJobResult, JobCancellation, JobResultDisposition, JobState, JobSupervisor, JobTicket,
};
pub use ocr::{recognize_image, recognize_image_with_cancellation, tesseract_available};
pub use ocr_job::{LocalOcrRunner, OcrJobCompletion, OcrJobService, OcrRunner};
pub use os_instance::OsSingleInstance;
pub use pin_window::{PinSessionEnd, PinView, run_pin_session, scaled_window_size};
pub use platform::{CapabilityProbe, FakeCapabilityProbe, RuntimeCapabilityProbe};
pub use region_overlay::run_region_selection;
pub use region_workflow::{RegionCaptureResult, capture_region_interactive};
pub use runtime::{AppRuntime, BootstrapOutcome, DispatchResult};
pub use single_instance::{
    InMemorySingleInstance, InstanceAcquisition, SingleInstance, SingleInstanceError,
};
