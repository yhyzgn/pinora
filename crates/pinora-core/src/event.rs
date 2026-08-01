use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::ErrorCode;
use crate::geometry::PixelSize;
use crate::ids::{CorrelationId, EventId, ImageId, PinId};

/// 已发生的领域事实类别。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEventKind {
    AppStarted,
    AppActivated,
    AppShuttingDown,
    AppStopped,
    CaptureCompleted { image_id: ImageId, size: PixelSize },
    PinCreated { pin_id: PinId, image_id: ImageId },
    PinClosed { pin_id: PinId },
    PinUpdated { pin_id: PinId },
    ImageSaved { image_id: ImageId, path: PathBuf },
    ImageCopied { image_id: ImageId },
    CommandFailed { code: ErrorCode, message: String },
}

/// 领域事件载荷（不含像素、OCR 全文或凭据）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEvent {
    pub kind: DomainEventKind,
}

/// 带诊断元数据的事件信封。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventEnvelope {
    pub event_id: EventId,
    pub correlation_id: CorrelationId,
    pub occurred_at_ms: u64,
    pub event: DomainEvent,
}

impl EventEnvelope {
    pub fn now(correlation_id: CorrelationId, event: DomainEvent) -> Self {
        Self {
            event_id: EventId::new(),
            correlation_id,
            occurred_at_ms: system_time_ms(),
            event,
        }
    }
}

fn system_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_contains_ids() {
        let corr = CorrelationId::from_raw(42);
        let env = EventEnvelope::now(
            corr,
            DomainEvent {
                kind: DomainEventKind::AppStarted,
            },
        );
        assert_eq!(env.correlation_id, corr);
        assert!(env.event_id.raw() > 0);
    }
}
