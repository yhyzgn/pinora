//! 本地诊断面板的纯视图模型与 XRGB 呈现。
//!
//! 本模块刻意不保存或读取 `CapabilitySnapshot.notes`。诊断内容只能来自稳定的
//! 能力布尔值、实际热键注册结果、本地 OCR 可用性和受控 tray 反馈。

use pinora_core::{CapabilitySnapshot, ErrorCode, PixelRect};

use crate::settings_panel::{draw_outline, draw_text, fill};
use crate::tray_feedback::TrayFeedback;

pub(crate) const PANEL_WIDTH: u32 = 540;
pub(crate) const PANEL_HEIGHT: u32 = 390;

const CAPABILITY_LABELS: [&str; 5] = [
    "CAPTURE",
    "GLOBAL HOTKEY",
    "IMAGE CLIPBOARD",
    "ALWAYS ON TOP",
    "LOCAL OCR",
];

/// 仅保存已经脱敏并受限的诊断状态；不保留运行时 notes 或原始错误字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiagnosticsPanel {
    platform: &'static str,
    capabilities: [bool; 5],
    feedback: TrayFeedback,
}

impl DiagnosticsPanel {
    /// 全局热键以 `GlobalHotkeyHub` 的实际注册结果为准，不能使用启动 probe 的
    /// 平台推测。其他值由能力快照的明确布尔字段提供。
    pub(crate) const fn from_runtime(
        runtime: &CapabilitySnapshot,
        global_hotkey_registered: bool,
        ocr_available: bool,
        feedback: TrayFeedback,
    ) -> Self {
        Self {
            platform: platform_label(),
            capabilities: [
                runtime.capture_available,
                global_hotkey_registered,
                runtime.clipboard_image_available,
                runtime.always_on_top_available,
                ocr_available,
            ],
            feedback,
        }
    }

    pub(crate) const fn platform(&self) -> &'static str {
        self.platform
    }

    pub(crate) const fn capability_rows(&self) -> [(&'static str, bool); 5] {
        [
            (CAPABILITY_LABELS[0], self.capabilities[0]),
            (CAPABILITY_LABELS[1], self.capabilities[1]),
            (CAPABILITY_LABELS[2], self.capabilities[2]),
            (CAPABILITY_LABELS[3], self.capabilities[3]),
            (CAPABILITY_LABELS[4], self.capabilities[4]),
        ]
    }

    pub(crate) const fn feedback_label(&self) -> &'static str {
        self.feedback.diagnostic_label()
    }

    pub(crate) const fn error_code(&self) -> Option<ErrorCode> {
        self.feedback.error_code()
    }

    pub(crate) const fn recovery_suggestion(&self) -> Option<&'static str> {
        match self.error_code() {
            Some(error) => Some(recovery_suggestion(error)),
            None => None,
        }
    }

    pub(crate) fn set_feedback(&mut self, feedback: TrayFeedback) {
        self.feedback = feedback;
    }
}

const fn availability_label(available: bool) -> &'static str {
    if available { "AVAILABLE" } else { "RESTRICTED" }
}

const fn recovery_suggestion(error: ErrorCode) -> &'static str {
    match error {
        ErrorCode::PermissionDenied => "CHECK SYSTEM PERMISSIONS THEN RETRY",
        ErrorCode::CapabilityUnavailable => "CHECK PLATFORM SUPPORT OR USE TRAY IPC",
        ErrorCode::RetryablePlatform | ErrorCode::TimedOut => "RETRY LATER RESTART IF PERSISTENT",
        ErrorCode::ClipboardFailed => "CHECK CLIPBOARD SERVICE THEN RETRY",
        ErrorCode::CommandRejected | ErrorCode::InvalidState => "FINISH CURRENT ACTION THEN RETRY",
        ErrorCode::ResourceLimitExceeded => "CLOSE UNUSED PINS THEN RETRY",
        ErrorCode::AlreadyRunning | ErrorCode::SingleInstanceBusy => {
            "USE THE RUNNING PINORA INSTANCE"
        }
        ErrorCode::NotRunning | ErrorCode::NotFound => "REOPEN THE REQUESTED ITEM",
        ErrorCode::Cancelled => "RUN THE ACTION AGAIN WHEN READY",
        ErrorCode::Internal => "RESTART PINORA IF THE ERROR PERSISTS",
    }
}

#[cfg(target_os = "windows")]
const fn platform_label() -> &'static str {
    "WINDOWS"
}

#[cfg(target_os = "macos")]
const fn platform_label() -> &'static str {
    "MACOS"
}

#[cfg(target_os = "linux")]
const fn platform_label() -> &'static str {
    "LINUX"
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
const fn platform_label() -> &'static str {
    "OTHER"
}

/// 将受控状态绘制到诊断窗口。所有行使用固定 ASCII，避免依赖平台字体和意外
/// 将用户来源文本带入像素缓冲。
pub(crate) fn paint(panel: &DiagnosticsPanel, frame: &mut [u32], stride: usize, height: usize) {
    fill(
        frame,
        stride,
        height,
        PixelRect::new(0, 0, PANEL_WIDTH, PANEL_HEIGHT),
        0x00131A24,
    );
    fill(
        frame,
        stride,
        height,
        PixelRect::new(0, 0, PANEL_WIDTH, 52),
        0x00213142,
    );
    draw_outline(
        frame,
        stride,
        height,
        PixelRect::new(0, 0, PANEL_WIDTH, PANEL_HEIGHT),
        0x005A718A,
    );
    draw_text(
        frame,
        stride,
        height,
        22,
        22,
        "PINORA DIAGNOSTICS",
        0x00E5EDF5,
    );
    draw_text(frame, stride, height, 22, 38, "LOCAL STATUS", 0x009DC4F0);

    draw_text(frame, stride, height, 24, 74, "PLATFORM", 0x00B9C7D8);
    draw_text(frame, stride, height, 220, 74, panel.platform(), 0x00E5EDF5);

    for (index, (label, available)) in panel.capability_rows().into_iter().enumerate() {
        let top = 96 + index as i32 * 36;
        let rect = PixelRect::new(18, top, PANEL_WIDTH - 36, 28);
        fill(frame, stride, height, rect, 0x001D2937);
        draw_outline(frame, stride, height, rect, 0x003F596E);
        draw_text(frame, stride, height, 30, top + 11, label, 0x00D4E0ED);
        let color = if available { 0x007DD7A0 } else { 0x00F0B777 };
        draw_text(
            frame,
            stride,
            height,
            320,
            top + 11,
            availability_label(available),
            color,
        );
    }

    let feedback_top = 280;
    draw_text(
        frame,
        stride,
        height,
        24,
        feedback_top,
        "RECENT STATUS",
        0x00B9C7D8,
    );
    draw_text(
        frame,
        stride,
        height,
        24,
        feedback_top + 18,
        panel.feedback_label(),
        0x00E5EDF5,
    );
    if let Some(error) = panel.error_code() {
        draw_text(
            frame,
            stride,
            height,
            24,
            feedback_top + 48,
            "ERROR CODE",
            0x00F0B777,
        );
        draw_text(
            frame,
            stride,
            height,
            120,
            feedback_top + 48,
            error.as_str(),
            0x00F0B777,
        );
    }
    if let Some(recovery) = panel.recovery_suggestion() {
        draw_text(
            frame,
            stride,
            height,
            24,
            feedback_top + 72,
            "RECOVERY",
            0x009DC4F0,
        );
        draw_text(
            frame,
            stride,
            height,
            24,
            feedback_top + 90,
            recovery,
            0x00D4E0ED,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray_feedback::TrayExportOperation;

    fn runtime_with_notes(notes: Vec<String>) -> CapabilitySnapshot {
        CapabilitySnapshot {
            capture_available: false,
            global_hotkey_available: true,
            clipboard_image_available: false,
            always_on_top_available: true,
            notes,
        }
    }

    #[test]
    fn actual_hotkey_result_overrides_bootstrap_guess_and_notes_are_not_retained() {
        let panel = DiagnosticsPanel::from_runtime(
            &runtime_with_notes(vec!["/home/neo/secret.png\nbackend detail".into()]),
            false,
            true,
            TrayFeedback::Ready,
        );

        assert_eq!(
            panel.capability_rows(),
            [
                ("CAPTURE", false),
                ("GLOBAL HOTKEY", false),
                ("IMAGE CLIPBOARD", false),
                ("ALWAYS ON TOP", true),
                ("LOCAL OCR", true),
            ]
        );
        assert_eq!(panel.feedback_label(), "READY");
        assert_eq!(panel.error_code(), None);
        assert_ne!(panel.platform(), "/home/neo/secret.png\nbackend detail");
    }

    #[test]
    fn failure_has_stable_code_and_fixed_recovery_without_raw_message() {
        let panel = DiagnosticsPanel::from_runtime(
            &runtime_with_notes(vec!["OCR text: private content".into()]),
            true,
            false,
            TrayFeedback::ExportFailed(TrayExportOperation::CopyImage, ErrorCode::ClipboardFailed),
        );

        assert_eq!(panel.feedback_label(), "EXPORT FAILED");
        assert_eq!(panel.error_code(), Some(ErrorCode::ClipboardFailed));
        assert_eq!(
            panel.recovery_suggestion(),
            Some("CHECK CLIPBOARD SERVICE THEN RETRY")
        );
    }

    #[test]
    fn feedback_refresh_never_creates_an_error_for_successful_state() {
        let mut panel = DiagnosticsPanel::from_runtime(
            &CapabilitySnapshot::default(),
            false,
            false,
            TrayFeedback::CaptureFailed(ErrorCode::TimedOut),
        );
        panel.set_feedback(TrayFeedback::OcrCompleted);

        assert_eq!(panel.feedback_label(), "OCR COMPLETED");
        assert_eq!(panel.error_code(), None);
        assert_eq!(panel.recovery_suggestion(), None);
    }
}
