//! Pinora 导出边界：图像编码、文件发布、系统剪贴板与受监督导出任务。

mod capture_export;
mod clipboard_read;
mod export_contract;
mod export_job;
mod image_sink;

pub use capture_export::{CaptureExportSource, compose_capture_export_image};
pub use clipboard_read::{
    ClipboardImageReadCompletion, ClipboardImageReadJobService, ClipboardImageReader,
    LocalClipboardImageReader,
};
pub use export_contract::{
    ExportAction, ExportOperation, FrozenExportTarget, OverlayExportAction,
    capture_export_source_for_overlay_action,
};
pub use export_job::{
    ExportJobCompletion, ExportJobInput, ExportJobService, ExportRunner, LocalExportRunner,
};
pub use image_sink::{
    LocalImageSink, copy_text_to_system_clipboard, detect_system_clipboard_backend, save_png_file,
};
