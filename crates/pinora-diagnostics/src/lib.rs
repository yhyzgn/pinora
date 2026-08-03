//! 用户主动导出的脱敏诊断报告。
//!
//! 报告模型只接受固定标签、布尔值和数值，避免以后把原始路径、像素、OCR 或
//! 平台错误字符串意外写入诊断文件。文件写入沿用导出目录的同目录临时文件协议。

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use pinora_core::{AppSettings, ErrorCode, ExportImageFormat, ThemeMode};

static REPORT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const REPORT_PREFIX: &str = "pinora-diagnostics";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SanitizedSettings {
    pub schema_version: u16,
    pub theme: &'static str,
    pub start_on_login: bool,
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
    const fn from_settings(settings: AppSettings) -> Self {
        Self {
            schema_version: settings.schema_version,
            theme: theme_label(settings.theme),
            start_on_login: settings.start_on_login,
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

/// 创建诊断报告所需的已脱敏状态。
///
/// 能力数组的固定顺序为捕获、全局热键、图像剪贴板、置顶和本地 OCR。调用方不能为
/// 报告自定义字段名，避免将未审计的运行时文本写入磁盘。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticReportInput {
    platform: &'static str,
    capabilities: [bool; 5],
    feedback: &'static str,
    error_code: Option<ErrorCode>,
    settings: AppSettings,
}

impl DiagnosticReportInput {
    pub fn new(
        platform: &'static str,
        capabilities: [bool; 5],
        feedback: &'static str,
        error_code: Option<ErrorCode>,
        settings: AppSettings,
    ) -> Option<Self> {
        let platform = fixed_platform_label(platform)?;
        let feedback = fixed_feedback_label(feedback)?;
        Some(Self {
            platform,
            capabilities,
            feedback,
            error_code,
            settings,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedDiagnosticReport {
    platform: &'static str,
    capabilities: [(&'static str, bool); 5],
    feedback: &'static str,
    error_code: Option<ErrorCode>,
    settings: SanitizedSettings,
}

impl SanitizedDiagnosticReport {
    pub const fn from_input(input: DiagnosticReportInput) -> Self {
        Self {
            platform: input.platform,
            capabilities: [
                ("CAPTURE", input.capabilities[0]),
                ("GLOBAL HOTKEY", input.capabilities[1]),
                ("IMAGE CLIPBOARD", input.capabilities[2]),
                ("ALWAYS ON TOP", input.capabilities[3]),
                ("LOCAL OCR", input.capabilities[4]),
            ],
            feedback: input.feedback,
            error_code: input.error_code,
            settings: SanitizedSettings::from_settings(input.settings),
        }
    }

    pub fn render(&self) -> String {
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
        push_bool_line(
            &mut report,
            "settings.start_on_login",
            self.settings.start_on_login,
        );
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

pub fn write_report(
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

fn fixed_platform_label(platform: &str) -> Option<&'static str> {
    match platform {
        "WINDOWS" => Some("WINDOWS"),
        "MACOS" => Some("MACOS"),
        "LINUX" => Some("LINUX"),
        "OTHER" => Some("OTHER"),
        _ => None,
    }
}

fn fixed_feedback_label(feedback: &str) -> Option<&'static str> {
    match feedback {
        "READY" => Some("READY"),
        "CAPTURE PREPARING" => Some("CAPTURE PREPARING"),
        "CAPTURE READY" => Some("CAPTURE READY"),
        "CAPTURE CANCELLED" => Some("CAPTURE CANCELLED"),
        "CAPTURE FAILED" => Some("CAPTURE FAILED"),
        "DELAYED CAPTURE ACTIVE" => Some("DELAYED CAPTURE ACTIVE"),
        "DELAYED CAPTURE CANCELLED" => Some("DELAYED CAPTURE CANCELLED"),
        "DELAYED CAPTURE FAILED" => Some("DELAYED CAPTURE FAILED"),
        "OCR RUNNING" => Some("OCR RUNNING"),
        "OCR COMPLETED" => Some("OCR COMPLETED"),
        "OCR FAILED" => Some("OCR FAILED"),
        "EXPORT RUNNING" => Some("EXPORT RUNNING"),
        "EXPORT CANCELLING" => Some("EXPORT CANCELLING"),
        "EXPORT CANCELLED" => Some("EXPORT CANCELLED"),
        "EXPORT COMPLETED" => Some("EXPORT COMPLETED"),
        "EXPORT FAILED" => Some("EXPORT FAILED"),
        "PIN MOUSE PASSTHROUGH ENABLED" => Some("PIN MOUSE PASSTHROUGH ENABLED"),
        "PIN MOUSE INTERACTION RESTORED" => Some("PIN MOUSE INTERACTION RESTORED"),
        "PIN MOUSE PASSTHROUGH UNAVAILABLE" => Some("PIN MOUSE PASSTHROUGH UNAVAILABLE"),
        "DIAGNOSTICS EXPORTED" => Some("DIAGNOSTICS EXPORTED"),
        "DIAGNOSTICS EXPORT FAILED" => Some("DIAGNOSTICS EXPORT FAILED"),
        _ => None,
    }
}

const fn theme_label(theme: ThemeMode) -> &'static str {
    match theme {
        ThemeMode::System => "system",
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
    }
}

const fn export_format_label(format: ExportImageFormat) -> &'static str {
    match format {
        ExportImageFormat::Png => "png",
        ExportImageFormat::Jpeg => "jpeg",
        ExportImageFormat::WebP => "webp",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn report() -> SanitizedDiagnosticReport {
        SanitizedDiagnosticReport::from_input(
            DiagnosticReportInput::new(
                "LINUX",
                [true, true, true, false, true],
                "READY",
                None,
                AppSettings::default(),
            )
            .expect("fixed diagnostic input"),
        )
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
    fn input_rejects_unreviewed_platform_and_feedback_labels() {
        assert!(
            DiagnosticReportInput::new(
                "SECRET PLATFORM",
                [false; 5],
                "READY",
                None,
                AppSettings::default(),
            )
            .is_none()
        );
        assert!(
            DiagnosticReportInput::new(
                "LINUX",
                [false; 5],
                "RAW PLATFORM ERROR",
                None,
                AppSettings::default(),
            )
            .is_none()
        );
    }

    #[test]
    fn rendered_report_keeps_the_stable_field_order() {
        let rendered = report().render();
        let keys: Vec<_> = rendered
            .lines()
            .map(|line| line.split_once('=').map_or(line, |(key, _)| key))
            .collect();
        assert_eq!(
            keys,
            vec![
                "PINORA_DIAGNOSTICS_V1",
                "version",
                "platform",
                "capability.capture",
                "capability.global_hotkey",
                "capability.image_clipboard",
                "capability.always_on_top",
                "capability.local_ocr",
                "feedback",
                "error_code",
                "settings.schema_version",
                "settings.theme",
                "settings.start_on_login",
                "settings.history_limit",
                "settings.history_retention_days",
                "settings.history_max_bytes",
                "settings.pin_limit",
                "settings.default_pin_opacity_percent",
                "settings.default_pin_always_on_top",
                "settings.ocr_confidence_threshold",
                "settings.export_format",
                "settings.jpeg_quality",
            ]
        );
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
            start_on_login: true,
            ..AppSettings::default()
        };
        let summary = SanitizedSettings::from_settings(settings);
        assert_eq!(summary.theme, "dark");
        assert_eq!(summary.export_format, "png");
        assert!(summary.start_on_login);
        let report = SanitizedDiagnosticReport::from_input(
            DiagnosticReportInput::new("LINUX", [false; 5], "READY", None, settings)
                .expect("fixed diagnostic input"),
        );
        assert!(report.render().contains("settings.start_on_login=true\n"));
    }
}
