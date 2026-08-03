//! 历史保留期的时间策略。
//!
//! 此模块只把保留天数转换为 Unix 毫秒截止点；何时执行历史策略、如何持久化和如何反馈错误
//! 均由调用方处理。

use std::time::{SystemTime, UNIX_EPOCH};

const MILLIS_PER_DAY: u64 = 86_400_000;

/// 基于当前 Unix 时间计算保留期截止点。
///
/// 零天表示不应用基于时间的清理。系统时间早于 Unix epoch 或无法表示为 `u64` 毫秒时，
/// 返回 `None`，避免在不可靠时钟下意外清理历史。
pub fn history_retention_cutoff_ms(retention_days: u16) -> Option<u64> {
    retention_cutoff_from_clock(retention_days, current_unix_epoch_ms)
}

/// 基于调用方提供的 Unix 毫秒时间计算截止点。
fn history_retention_cutoff_from_now(now_ms: u64, retention_days: u16) -> Option<u64> {
    if retention_days == 0 {
        return None;
    }
    let retention_ms = u64::from(retention_days).checked_mul(MILLIS_PER_DAY)?;
    Some(now_ms.saturating_sub(retention_ms))
}

fn retention_cutoff_from_clock(
    retention_days: u16,
    now: impl FnOnce() -> Option<u64>,
) -> Option<u64> {
    if retention_days == 0 {
        return None;
    }
    history_retention_cutoff_from_now(now()?, retention_days)
}

fn current_unix_epoch_ms() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| duration.as_millis().try_into().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_days_does_not_produce_a_cutoff() {
        assert_eq!(history_retention_cutoff_from_now(100, 0), None);
    }

    #[test]
    fn cutoff_moves_back_by_whole_days() {
        assert_eq!(
            history_retention_cutoff_from_now(40 * MILLIS_PER_DAY, 30),
            Some(10 * MILLIS_PER_DAY)
        );
    }

    #[test]
    fn early_clock_saturates_at_epoch() {
        assert_eq!(history_retention_cutoff_from_now(1, 30), Some(0));
    }

    #[test]
    fn maximum_u16_day_count_stays_representable() {
        assert_eq!(
            history_retention_cutoff_from_now(u64::MAX, u16::MAX),
            Some(
                u64::MAX
                    - u64::from(u16::MAX)
                        .checked_mul(MILLIS_PER_DAY)
                        .expect("u16 day duration fits in u64")
            )
        );
    }

    #[test]
    fn unavailable_clock_does_not_produce_a_cutoff() {
        assert_eq!(retention_cutoff_from_clock(30, || None), None);
    }

    #[test]
    fn zero_days_does_not_read_the_clock() {
        assert_eq!(
            retention_cutoff_from_clock(0, || panic!("zero days must not require a clock")),
            None
        );
    }
}
