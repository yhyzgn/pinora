//! Pinora 应用编排：生命周期、单实例与命令分发。

mod capture_fake;
mod capture_kde;
mod capture_select;
mod capture_xcap;
mod desktop_shell;
mod diagnostics_export;
mod diagnostics_panel;
mod diagnostics_window;
mod export_job;
mod export_name;
mod frame_cache;
mod history_browser;
mod history_export;
mod history_load_job;
mod history_store;
mod history_window;
mod image_sink;
mod job_supervisor;
mod kwin_place;
mod ocr;
mod ocr_job;
mod ocr_presentation;
mod overlay_preview_cache;
mod overlay_selection_readout;
mod overlay_toolbar;
mod panel_theme;
mod pin_context_menu;
mod pin_layout;
mod platform;
mod runtime;
mod settings_panel;
mod settings_store;
mod settings_window;
mod tray;
mod tray_capabilities;
mod tray_feedback;
mod window_policy;
mod worker_lifecycle;

pub use capture_fake::FakeCaptureProvider;
pub use capture_kde::KdeSpectacleCaptureProvider;
pub use capture_select::{CaptureBackendKind, SelectedCaptureProvider, fake_only};
pub use capture_xcap::XcapCaptureProvider;
pub use desktop_shell::run_desktop_shell;
pub use export_job::{
    ExportJobCompletion, ExportJobInput, ExportJobService, ExportRunner, LocalExportRunner,
};
pub use history_browser::{HistoryPanel, HistoryPanelAction, HistoryPanelKey, HistoryPanelStatus};
pub use history_store::{HistoryLoad, HistoryStore, default_history_path};
pub use image_sink::{
    LocalImageSink, copy_text_to_system_clipboard, detect_system_clipboard_backend,
};
pub use job_supervisor::{
    AcceptedJobResult, JobCancellation, JobResultDisposition, JobState, JobSupervisor, JobTicket,
};
pub use ocr::{recognize_image, recognize_image_with_cancellation, tesseract_available};
pub use ocr_job::{LocalOcrRunner, OcrJobCompletion, OcrJobService, OcrJobStart, OcrRunner};
pub use pin_layout::scaled_window_size;
pub use platform::{CapabilityProbe, FakeCapabilityProbe, RuntimeCapabilityProbe};
pub use runtime::{AppRuntime, BootstrapOutcome, DispatchResult};
pub use settings_panel::{
    SettingField, SettingsPanel, SettingsPanelAction, SettingsPanelKey, SettingsPanelStatus,
};
pub use settings_store::{SettingsLoad, SettingsStore, default_settings_path};
