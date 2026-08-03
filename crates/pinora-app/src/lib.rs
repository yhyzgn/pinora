//! Pinora 应用编排：生命周期、单实例与命令分发。

mod desktop_shell;
mod diagnostics_export;
mod diagnostics_window;
mod history_window;
mod platform;
mod settings_window;

pub(crate) use pinora_desktop::{
    diagnostics_panel, history_browser, overlay_selection_readout, panel_theme, pin_context_menu,
    settings_panel, tray_capabilities, tray_feedback,
};
pub(crate) use pinora_history::{
    HistoryExportCandidate, HistoryLoadCompletion, HistoryLoadInput, HistoryLoadJobService,
    HistoryLoadPayload, HistoryLoadPreparation, clear_history_entries, delete_history_entry,
    history_candidate_for_export, load_history_index, reconcile_history_policy,
    record_history_candidate,
};
pub(crate) use pinora_tray::{AppTray, TrayAction, TrayPinListEntry};

pub use desktop_shell::run_desktop_shell;
pub use pinora_capture::{
    CachedFrame, CaptureBackendKind, FakeCaptureProvider, FrameCache, KdeSpectacleCaptureProvider,
    SelectedCaptureProvider, XcapCaptureProvider, fake_only, rgba_to_xrgb, rgba_to_xrgb_and_dim,
};
pub use pinora_desktop::history_browser::{
    HistoryPanel, HistoryPanelAction, HistoryPanelKey, HistoryPanelStatus,
};
pub use pinora_desktop::scaled_window_size;
pub use pinora_desktop::settings_panel::{
    SettingField, SettingsPanel, SettingsPanelAction, SettingsPanelKey, SettingsPanelStatus,
};
pub use pinora_export::{
    ExportJobCompletion, ExportJobInput, ExportJobService, ExportRunner, LocalExportRunner,
    LocalImageSink, copy_text_to_system_clipboard, detect_system_clipboard_backend,
};
pub use pinora_jobs::{
    AcceptedJobResult, JobCancellation, JobResultDisposition, JobState, JobSupervisor, JobTicket,
};
pub use pinora_ocr::{LocalOcrRunner, OcrJobCompletion, OcrJobService, OcrJobStart, OcrRunner};
pub use pinora_ocr::{recognize_image, recognize_image_with_cancellation, tesseract_available};
pub use pinora_runtime::{AppRuntime, BootstrapOutcome, CapabilityProbe, DispatchResult};
pub use pinora_storage::{
    ExportNameAllocator, HistoryLoad, HistoryStore, SettingsLoad, SettingsStore,
    default_history_path, default_settings_path,
};
pub use platform::{FakeCapabilityProbe, RuntimeCapabilityProbe};
