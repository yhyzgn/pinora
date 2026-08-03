//! Pinora 应用编排：生命周期、单实例与命令分发。

mod desktop_shell;
mod diagnostics_export;
mod diagnostics_panel;
mod diagnostics_window;
mod export_job;
mod history_browser;
mod history_export;
mod history_load_job;
mod history_window;
mod image_sink;
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
mod settings_window;
mod tray;
mod tray_capabilities;
mod tray_feedback;
mod window_policy;

pub use desktop_shell::run_desktop_shell;
pub use export_job::{
    ExportJobCompletion, ExportJobInput, ExportJobService, ExportRunner, LocalExportRunner,
};
pub use history_browser::{HistoryPanel, HistoryPanelAction, HistoryPanelKey, HistoryPanelStatus};
pub use image_sink::{
    LocalImageSink, copy_text_to_system_clipboard, detect_system_clipboard_backend,
};
pub use ocr::{recognize_image, recognize_image_with_cancellation, tesseract_available};
pub use ocr_job::{LocalOcrRunner, OcrJobCompletion, OcrJobService, OcrJobStart, OcrRunner};
pub use pin_layout::scaled_window_size;
pub use pinora_capture::{
    CachedFrame, CaptureBackendKind, FakeCaptureProvider, FrameCache, KdeSpectacleCaptureProvider,
    SelectedCaptureProvider, XcapCaptureProvider, fake_only, rgba_to_xrgb, rgba_to_xrgb_and_dim,
};
pub use pinora_jobs::{
    AcceptedJobResult, JobCancellation, JobResultDisposition, JobState, JobSupervisor, JobTicket,
};
pub use pinora_storage::{
    ExportNameAllocator, HistoryLoad, HistoryStore, SettingsLoad, SettingsStore,
    default_history_path, default_settings_path,
};
pub use platform::{CapabilityProbe, FakeCapabilityProbe, RuntimeCapabilityProbe};
pub use runtime::{AppRuntime, BootstrapOutcome, DispatchResult};
pub use settings_panel::{
    SettingField, SettingsPanel, SettingsPanelAction, SettingsPanelKey, SettingsPanelStatus,
};
