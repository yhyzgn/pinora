use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// 关联一次用户意图或工作流的标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CorrelationId(u64);

/// 领域事件唯一标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventId(u64);

/// 截图图像标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageId(u64);

/// 贴图实体标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PinId(u64);

static NEXT_CORRELATION: AtomicU64 = AtomicU64::new(1);
static NEXT_EVENT: AtomicU64 = AtomicU64::new(1);
static NEXT_IMAGE: AtomicU64 = AtomicU64::new(1);
static NEXT_PIN: AtomicU64 = AtomicU64::new(1);

macro_rules! id_impl {
    ($name:ident, $counter:ident, $prefix:literal) => {
        impl $name {
            pub fn new() -> Self {
                Self($counter.fetch_add(1, Ordering::Relaxed))
            }

            pub fn from_raw(value: u64) -> Self {
                Self(value)
            }

            pub fn raw(self) -> u64 {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "-{}"), self.0)
            }
        }
    };
}

id_impl!(CorrelationId, NEXT_CORRELATION, "corr");
id_impl!(EventId, NEXT_EVENT, "evt");
id_impl!(ImageId, NEXT_IMAGE, "img");
id_impl!(PinId, NEXT_PIN, "pin");

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

        let i1 = ImageId::new();
        let i2 = ImageId::new();
        assert_ne!(i1, i2);

        let p1 = PinId::new();
        let p2 = PinId::new();
        assert_ne!(p1, p2);
    }
}
