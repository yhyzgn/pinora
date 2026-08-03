//! Pinora 领域核心：纯数据类型、命令、事件与错误码。
//!
//! 本 crate 不得依赖 UI 框架或平台 SDK。

mod action;
mod annotate;
mod asset;
mod capture;
mod command;
mod error;
mod event;
mod export;
mod geometry;
mod history;
mod ids;
mod image;
mod job;
mod ocr;
mod pin;
mod selection;
mod settings;
mod state;

pub use action::{ActionId, KeyBinding};
pub use annotate::{
    AnnotateSession, AnnotateTool, Annotation, AnnotationDoc, AnnotationRevision, DEFAULT_STROKE,
    DEFAULT_WIDTH, DraftShape, MAX_SEQUENCE_NUMBER, MAX_STROKE, MIN_SEQUENCE_NUMBER, MIN_STROKE,
    STROKE_PALETTE, bake_annotations, color_to_hex, render_annotation_rgba, render_draft_rgba,
    render_preview_rgba, sample_rgba_at,
};
pub use asset::{AssetGeneration, AssetRef};
pub use capture::{
    CaptureProvider, CaptureRequest, CaptureWindowId, CaptureWindowInfo, DisplayInfo,
    resolve_all_displays_rect, resolve_capture_rect,
};
pub use command::Command;
pub use error::{ErrorCode, PinoraError};
pub use event::{DomainEvent, DomainEventKind, EventEnvelope};
pub use export::ImageSink;
pub use geometry::{PixelPoint, PixelRect, PixelSize};
pub use history::{
    ContentDigest, HISTORY_MAX_DISPLAY_BYTES, HISTORY_MAX_ENTRIES, HISTORY_MAX_FILE_NAME_BYTES,
    HISTORY_SCHEMA_VERSION, HistoryEntry, HistoryEntrySpec, HistoryEntryState, HistoryIndex,
    HistoryInsert, HistoryOcrState,
};
pub use ids::{CorrelationId, EventId, ImageId, JobId, PinId, SessionId};
pub use image::{CaptureImage, CaptureMetadata, DisplayId, RgbaBuffer};
pub use job::{JobKind, JobOwner, JobResultRef, JobSpec, JobTerminalState};
pub use ocr::{
    OcrLine, OcrResult, OcrTextSelection, OcrWord, OcrWordRef, join_lines_text, union_bboxes,
};
pub use pin::{Pin, PinMode, PinTransform};
pub use selection::{
    MIN_SELECTION_EDGE, SelectionHandle, SelectionOutcome, SelectionSession, clamp_to_image,
    normalize_rect, validate_min_size,
};
pub use settings::{
    AppSettings, DEFAULT_FULL_DISPLAY_HOTKEY, DEFAULT_HISTORY_LIMIT,
    DEFAULT_OCR_CONFIDENCE_THRESHOLD, DEFAULT_PIN_ALWAYS_ON_TOP, DEFAULT_PIN_LIMIT,
    DEFAULT_PIN_OPACITY_PERCENT, DEFAULT_REGION_HOTKEY, HotkeyBinding, HotkeyCode, HotkeyModifiers,
    OcrLanguage, REGION_ALTERNATE_HOTKEY, REGION_SECONDARY_HOTKEY, SETTINGS_SCHEMA_VERSION,
    SettingsRepairs, ThemeMode,
};
pub use state::{AppPhase, AppState, CapabilitySnapshot};
