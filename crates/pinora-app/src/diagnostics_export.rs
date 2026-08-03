//! 用户主动导出的脱敏诊断报告。
//!
//! 报告模型只接受固定标签、布尔值和数值，避免以后把原始路径、像素、OCR 或
//! 平台错误字符串意外写入诊断文件。文件写入沿用导出目录的同目录临时文件协议。

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use pinora_core::{AppSettings, CapabilitySnapshot, ErrorCode};

use crate::diagnostics_panel::DiagnosticsPanel;

static REPORT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const REPORT_PREFIX: &str = "pinora-diagnostics";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SanitizedSettings {
    pub schema_version: u16,
    pub theme: &'static str,
    pub history_limit: u32,
    pub history_retention_days: u16,
    pub history_max_bytes: u64,
    pub pin_limit: u16,
    pub default_pin_opacity_percent: u8,
    pub default_pin_always_on_top: bool,
    pub ocr_confidence_threshold: u8,
    pub export_format: &'static str,
    pub jpeg_quality: u8,
}

impl SanitizedSettings {
    pub(crate) const fn from_settings(settings: AppSettings) -> Self {
        Self {
            schema_version: settings.schema_version,
            theme: theme_label(settings.theme),
            history_limit: settings.history_limit,
            history_retention_days: settings.history_retention_days,
            history_max_bytes: settings.history_max_bytes,
            pin_limit: settings.pin_limit,
            default_pin_opacity_percent: settings.default_pin_opacity_percent,
            default_pin_always_on_top: settings.default_pin_always_on_top,
            ocr_confidence_threshold: settings.ocr_confidence_threshold,
            export_format: export_format_label(settings.export_format),
            jpeg_quality: settings.jpeg_quality,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SanitizedDiagnosticReport {
    platform: &'static str,
    capabilities: [(&'static str, bool); 5],
    feedback: &'static str,
    error_code: Option<ErrorCode>,
    settings: SanitizedSettings,
}

impl SanitizedDiagnosticReport {
    pub(crate) fn from_runtime(
        capabilities: &CapabilitySnapshot,
        panel: &DiagnosticsPanel,
        settings: AppSettings,
    ) -> Self {
        let rows = panel.capability_rows();
        Self {
            platform: panel.platform(),
            capabilities: [
                (rows[0].0, capabilities.capture_available),
                (rows[1].0, rows[1].1),
                (rows[2].0, capabilities.clipboard_image_available),
                (rows[3].0, capabilities.always_on_top_available),
                (rows[4].0, rows[4].1),
            ],
            feedback: panel.feedback_label(),
            error_code: panel.error_code(),
            settings: SanitizedSettings::from_settings(settings),
        }
    }

    pub(crate) fn render(&self) -> String {
        let mut report = String::with_capacity(1_600);
        report.push_str("PINORA_DIAGNOSTICS_V1\n");
        push_line(&mut report, "version", env!("CARGO_PKG_VERSION"));
        push_line(&mut report, "platform", self.platform);
        for (label, available) in self.capabilities {
            push_bool_line(&mut report, capability_key(label), available);
        }
        push_line(&mut report, "feedback", self.feedback);
        push_line(
            &mut report,
            "error_code",
            self.error_code.map_or("none", ErrorCode::as_str),
        );
        push_line(
            &mut report,
            "settings.schema_version",
            &self.settings.schema_version.to_string(),
        );
        push_line(&mut report, "settings.theme", self.settings.theme);
        push_line(
            &mut report,
            "settings.history_limit",
            &self.settings.history_limit.to_string(),
        );
        push_line(
            &mut report,
            "settings.history_retention_days",
            &self.settings.history_retention_days.to_string(),
        );
        push_line(
            &mut report,
            "settings.history_max_bytes",
            &self.settings.history_max_bytes.to_string(),
        );
        push_line(
            &mut report,
            "settings.pin_limit",
            &self.settings.pin_limit.to_string(),
        );
        push_line(
            &mut report,
            "settings.default_pin_opacity_percent",
            &self.settings.default_pin_opacity_percent.to_string(),
        );
        push_bool_line(
            &mut report,
            "settings.default_pin_always_on_top",
            self.settings.default_pin_always_on_top,
        );
        push_line(
            &mut report,
            "settings.ocr_confidence_threshold",
            &self.settings.ocr_confidence_threshold.to_string(),
        );
        push_line(
            &mut report,
            "settings.export_format",
            self.settings.export_format,
        );
        push_line(
            &mut report,
            "settings.jpeg_quality",
            &self.settings.jpeg_quality.to_string(),
        );
        report
    }
}

pub(crate) fn write_report(
    directory: &Path,
    report: &SanitizedDiagnosticReport,
) -> Result<PathBuf, &'static str> {
    fs::create_dir_all(directory).map_err(|_| "diagnostic directory unavailable")?;
    let sequence = REPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| duration.as_millis().try_into().ok())
        .unwrap_or(0_u64);
    let stem = format!("{REPORT_PREFIX}-{timestamp}-{}", sequence);
    let target = directory.join(format!("{stem}.txt"));
    let temporary = directory.join(format!(".{stem}.tmp"));

    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| "diagnostic temporary file unavailable")?;
        file.write_all(report.render().as_bytes())
            .map_err(|_| "diagnostic report write failed")?;
        file.sync_all()
            .map_err(|_| "diagnostic report sync failed")?;
        drop(file);
        fs::rename(&temporary, &target).map_err(|_| "diagnostic report publish failed")?;
        let readable = File::open(&target)
            .and_then(|file| file.metadata())
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false);
        if !readable {
            return Err("diagnostic report verification failed");
        }
        Ok(target)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn push_line(report: &mut String, key: &str, value: &str) {
    report.push_str(key);
    report.push('=');
    report.push_str(value);
    report.push('\n');
}

fn push_bool_line(report: &mut String, key: &str, value: bool) {
    push_line(report, key, if value { "true" } else { "false" });
}

fn capability_key(label: &str) -> &'static str {
    match label {
        "CAPTURE" => "capability.capture",
        "GLOBAL HOTKEY" => "capability.global_hotkey",
        "IMAGE CLIPBOARD" => "capability.image_clipboard",
        "ALWAYS ON TOP" => "capability.always_on_top",
        "LOCAL OCR" => "capability.local_ocr",
        _ => "capability.unknown",
    }
}

const fn theme_label(theme: pinora_core::ThemeMode) -> &'static str {
    match theme {
        pinora_core::ThemeMode::System => "system",
        pinora_core::ThemeMode::Light => "light",
        pinora_core::ThemeMode::Dark => "dark",
    }
}

const fn export_format_label(format: pinora_core::ExportImageFormat) -> &'static str {
    match format {
        pinora_core::ExportImageFormat::Png => "png",
        pinora_core::ExportImageFormat::Jpeg => "jpeg",
        pinora_core::ExportImageFormat::WebP => "webp",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::ThemeMode;
    use std::fs;

    fn report() -> SanitizedDiagnosticReport {
        let capabilities = CapabilitySnapshot {
            capture_available: true,
            clipboard_image_available: true,
            ..CapabilitySnapshot::default()
        };
        let panel = DiagnosticsPanel::from_runtime(
            &capabilities,
            true,
            true,
            crate::tray_feedback::TrayFeedback::Ready,
        );
        SanitizedDiagnosticReport::from_runtime(&capabilities, &panel, AppSettings::default())
    }

    #[test]
    fn rendered_report_has_only_stable_sanitized_fields() {
        let rendered = report().render();
        assert!(rendered.starts_with("PINORA_DIAGNOSTICS_V1\n"));
        assert!(rendered.contains("capability.capture=true\n"));
        assert!(rendered.contains("error_code=none\n"));
        assert!(!rendered.contains("/home/"));
        assert!(!rendered.contains("ocr text"));
        assert!(rendered.contains("capability.image_clipboard=true\n"));
        assert!(!rendered.contains("clipboard_content"));
        assert!(!rendered.contains("token"));
    }

    #[test]
    fn report_writer_publishes_a_readable_file_and_cleans_temp_files() {
        let directory =
            std::env::temp_dir().join(format!("pinora-diagnostics-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let target = write_report(&directory, &report()).expect("write report");
        let contents = fs::read_to_string(&target).expect("read report");
        assert!(contents.contains("PINORA_DIAGNOSTICS_V1"));
        assert!(
            fs::read_dir(&directory)
                .expect("list report directory")
                .all(|entry| !entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".tmp"))
        );
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn sanitized_settings_keep_enum_labels_stable() {
        let settings = AppSettings {
            theme: ThemeMode::Dark,
            ..AppSettings::default()
        };
        let summary = SanitizedSettings::from_settings(settings);
        assert_eq!(summary.theme, "dark");
        assert_eq!(summary.export_format, "png");
    }
}
