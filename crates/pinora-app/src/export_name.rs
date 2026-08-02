//! 桌面受管 PNG 导出的可读文件名分配。
//!
//! 这里只生成位于调用方目录下的固定 ASCII 名称。它不编码图像、不写文件，也不
//! 读取截图、OCR、窗口或路径以外的用户数据。

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use pinora_core::{ErrorCode, PinoraError};

const MAX_SEQUENCE: u32 = 9_999;

/// 单个桌面进程内的 PNG 文件名分配器。
#[derive(Debug, Default)]
pub(crate) struct ExportNameAllocator {
    last_second: Option<u64>,
    next_sequence: u32,
}

impl ExportNameAllocator {
    /// 分配 `Pinora_YYYYMMDD_HHMMSS[_NNN].png`，并跳过调用方报告的已有候选。
    pub(crate) fn allocate<F>(
        &mut self,
        export_dir: &Path,
        now: SystemTime,
        mut occupied: F,
    ) -> Result<PathBuf, PinoraError>
    where
        F: FnMut(&Path) -> Result<bool, PinoraError>,
    {
        let seconds = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let first_sequence = if self.last_second == Some(seconds) {
            self.next_sequence
        } else {
            0
        };

        for sequence in first_sequence..=MAX_SEQUENCE {
            let path = export_dir.join(file_name(seconds, sequence));
            if !occupied(&path)? {
                self.last_second = Some(seconds);
                self.next_sequence = sequence.saturating_add(1);
                return Ok(path);
            }
        }

        Err(PinoraError::new(
            ErrorCode::CommandRejected,
            "managed export file name limit reached",
        ))
    }
}

fn file_name(seconds: u64, sequence: u32) -> String {
    let (year, month, day, hour, minute, second) = utc_parts(seconds);
    if sequence == 0 {
        format!("Pinora_{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}.png")
    } else {
        format!(
            "Pinora_{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}_{sequence:03}.png"
        )
    }
}

fn utc_parts(seconds: u64) -> (i32, u32, u32, u64, u64, u64) {
    let days = (seconds / 86_400).min(i64::MAX as u64) as i64;
    let (year, month, day) = civil_from_days(days);
    let day_seconds = seconds % 86_400;
    (
        year,
        month,
        day,
        day_seconds / 3_600,
        (day_seconds % 3_600) / 60,
        day_seconds % 60,
    )
}

// Howard Hinnant's civil-date conversion, with days since 1970-01-01 as input.
fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch.saturating_add(719_468);
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::time::Duration;

    use super::*;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn filename_uses_sortable_utc_time_without_internal_identity() {
        assert_eq!(file_name(0, 0), "Pinora_19700101_000000.png");
        assert_eq!(file_name(951_782_400, 0), "Pinora_20000229_000000.png");
        assert_eq!(
            file_name(1_785_681_000, 7),
            "Pinora_20260802_143000_007.png"
        );
        assert!(!file_name(1_785_681_000, 0).contains("ImageId"));
    }

    #[test]
    fn same_second_and_existing_names_increment_without_overwrite() {
        let directory = Path::new("/managed/export");
        let mut allocator = ExportNameAllocator::default();
        let mut existing = BTreeSet::from([directory.join("Pinora_20260802_143000.png")]);

        let first = allocator
            .allocate(directory, at(1_785_681_000), |path| {
                Ok(existing.contains(path))
            })
            .expect("allocate first");
        existing.insert(first.clone());
        let second = allocator
            .allocate(directory, at(1_785_681_000), |path| {
                Ok(existing.contains(path))
            })
            .expect("allocate second");

        assert_eq!(
            first.file_name().and_then(|name| name.to_str()),
            Some("Pinora_20260802_143000_001.png")
        );
        assert_eq!(
            second.file_name().and_then(|name| name.to_str()),
            Some("Pinora_20260802_143000_002.png")
        );
        assert_ne!(first, second);
    }

    #[test]
    fn occupied_check_failure_is_returned_without_advancing_state() {
        let directory = Path::new("/managed/export");
        let mut allocator = ExportNameAllocator::default();
        let error = allocator
            .allocate(directory, at(0), |_| {
                Err(PinoraError::new(
                    ErrorCode::Internal,
                    "occupied check failed",
                ))
            })
            .expect_err("must not allocate after failed check");
        assert_eq!(error.code, ErrorCode::Internal);

        let path = allocator
            .allocate(directory, at(0), |_| Ok(false))
            .expect("state remains unadvanced");
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("Pinora_19700101_000000.png")
        );
    }
}
