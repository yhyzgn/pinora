//! Pinora 领域核心：纯数据类型、命令、事件与错误码。
//!
//! 本 crate 不得依赖 UI 框架或平台 SDK。

mod capture;
mod command;
mod error;
mod event;
mod geometry;
mod ids;
mod image;
mod pin;
mod state;

pub use capture::{resolve_capture_rect, CaptureProvider, CaptureRequest, DisplayInfo};
pub use command::Command;
pub use error::{ErrorCode, PinoraError};
pub use event::{DomainEvent, DomainEventKind, EventEnvelope};
pub use geometry::{PixelPoint, PixelRect, PixelSize};
pub use ids::{CorrelationId, EventId, ImageId, PinId};
pub use image::{CaptureImage, CaptureMetadata, DisplayId, RgbaBuffer};
pub use pin::{Pin, PinMode, PinTransform};
pub use state::{AppPhase, AppState, CapabilitySnapshot};
