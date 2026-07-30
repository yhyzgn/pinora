//! Pinora 领域核心：纯数据类型、命令、事件与错误码。
//!
//! 本 crate 不得依赖 UI 框架或平台 SDK。

mod command;
mod error;
mod event;
mod ids;
mod state;

pub use command::Command;
pub use error::{ErrorCode, PinoraError};
pub use event::{DomainEvent, DomainEventKind, EventEnvelope};
pub use ids::{CorrelationId, EventId};
pub use state::{AppPhase, AppState, CapabilitySnapshot};
