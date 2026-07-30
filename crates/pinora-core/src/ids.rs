use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// 关联一次用户意图或工作流的标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CorrelationId(u64);

/// 领域事件唯一标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventId(u64);

static NEXT_CORRELATION: AtomicU64 = AtomicU64::new(1);
static NEXT_EVENT: AtomicU64 = AtomicU64::new(1);

impl CorrelationId {
    pub fn new() -> Self {
        Self(NEXT_CORRELATION.fetch_add(1, Ordering::Relaxed))
    }

    pub fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "corr-{}", self.0)
    }
}

impl EventId {
    pub fn new() -> Self {
        Self(NEXT_EVENT.fetch_add(1, Ordering::Relaxed))
    }

    pub fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "evt-{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_monotonic() {
        let a = CorrelationId::new();
        let b = CorrelationId::new();
        assert_ne!(a, b);
        assert!(b.raw() > a.raw());

        let e1 = EventId::new();
        let e2 = EventId::new();
        assert_ne!(e1, e2);
        assert!(e2.raw() > e1.raw());
    }
}
