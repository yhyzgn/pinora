//! Pinora 领域核心：纯数据类型、命令、事件与错误码。
//!
//! 本 crate 不得依赖 UI 框架或平台 SDK。

mod action;
mod capture;
mod command;
mod error;
mod event;
mod export;
mod geometry;
mod ids;
mod image;
mod pin;
mod selection;
mod state;

pub use action::{ActionId, KeyBinding};
pub use capture::{resolve_capture_rect, CaptureProvider, CaptureRequest, DisplayInfo};
pub use command::Command;
pub use error::{ErrorCode, PinoraError};
pub use event::{DomainEvent, DomainEventKind, EventEnvelope};
pub use export::ImageSink;
pub use geometry::{PixelPoint, PixelRect, PixelSize};
pub use ids::{CorrelationId, EventId, ImageId, PinId};
pub use image::{CaptureImage, CaptureMetadata, DisplayId, RgbaBuffer};
pub use pin::{Pin, PinMode, PinTransform};
pub use selection::{
    clamp_to_image, normalize_rect, validate_min_size, SelectionOutcome, SelectionSession,
    MIN_SELECTION_EDGE,
};
pub use state::{AppPhase, AppState, CapabilitySnapshot};
