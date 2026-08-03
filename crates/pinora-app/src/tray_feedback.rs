//! tray 中展示的最近操作状态。
//!
//! 文本只由有限枚举和稳定错误码映射生成，不能携带图像、OCR 文本、路径或原始
//! 后端错误，因此可安全地交给菜单项和图标 tooltip。

use pinora_core::ErrorCode;

/// 当前 tray 中可展示的导出操作种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayExportOperation {
    CopyImage,
    CopyText,
    SaveFile,
}

/// 当前 tray 中可展示的最近用户操作状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayFeedback {
    Ready,
    CapturePreparing,
    CaptureReady,
    CaptureCancelled,
    CaptureFailed(ErrorCode),
    DelayedCaptureScheduled,
    DelayedCaptureCancelled,
    DelayedCaptureFailed(ErrorCode),
    OcrRunning,
    OcrCompleted,
    OcrFailed(ErrorCode),
    ExportRunning(TrayExportOperation),
    ExportCompleted(TrayExportOperation),
    ExportFailed(TrayExportOperation, ErrorCode),
}

impl TrayFeedback {
    /// 菜单状态项和 tooltip 共享这一受控文本。
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Ready => "Pinora - 就绪",
            Self::CapturePreparing => "Pinora - 正在准备截图",
            Self::CaptureReady => "Pinora - 截图已就绪",
            Self::CaptureCancelled => "Pinora - 截图已取消",
            Self::CaptureFailed(error) => capture_failed_label(error),
            Self::DelayedCaptureScheduled => "Pinora - 延时截图已开始",
            Self::DelayedCaptureCancelled => "Pinora - 延时截图已取消",
            Self::DelayedCaptureFailed(error) => delayed_capture_failed_label(error),
            Self::OcrRunning => "Pinora - 正在识别文字",
            Self::OcrCompleted => "Pinora - 文字识别已完成",
            Self::OcrFailed(error) => ocr_failed_label(error),
            Self::ExportRunning(operation) => export_running_label(operation),
            Self::ExportCompleted(operation) => export_completed_label(operation),
            Self::ExportFailed(operation, error) => export_failed_label(operation, error),
        }
    }

    /// 诊断面板使用的短状态标识。它与 tray 文案一样只来自有限枚举，避免把
    /// 原始错误消息、路径或用户内容带入可见 UI。
    pub(crate) const fn diagnostic_label(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::CapturePreparing => "CAPTURE PREPARING",
            Self::CaptureReady => "CAPTURE READY",
            Self::CaptureCancelled => "CAPTURE CANCELLED",
            Self::CaptureFailed(_) => "CAPTURE FAILED",
            Self::DelayedCaptureScheduled => "DELAYED CAPTURE ACTIVE",
            Self::DelayedCaptureCancelled => "DELAYED CAPTURE CANCELLED",
            Self::DelayedCaptureFailed(_) => "DELAYED CAPTURE FAILED",
            Self::OcrRunning => "OCR RUNNING",
            Self::OcrCompleted => "OCR COMPLETED",
            Self::OcrFailed(_) => "OCR FAILED",
            Self::ExportRunning(_) => "EXPORT RUNNING",
            Self::ExportCompleted(_) => "EXPORT COMPLETED",
            Self::ExportFailed(_, _) => "EXPORT FAILED",
        }
    }

    /// 只有失败状态才向诊断面板提供稳定错误码；成功或进行中状态不伪造错误。
    pub(crate) const fn error_code(self) -> Option<ErrorCode> {
        match self {
            Self::CaptureFailed(error)
            | Self::DelayedCaptureFailed(error)
            | Self::OcrFailed(error)
            | Self::ExportFailed(_, error) => Some(error),
            Self::Ready
            | Self::CapturePreparing
            | Self::CaptureReady
            | Self::CaptureCancelled
            | Self::DelayedCaptureScheduled
            | Self::DelayedCaptureCancelled
            | Self::OcrRunning
            | Self::OcrCompleted
            | Self::ExportRunning(_)
            | Self::ExportCompleted(_) => None,
        }
    }
}

const fn capture_failed_label(error: ErrorCode) -> &'static str {
    match error {
        ErrorCode::PermissionDenied => "Pinora - 截图未完成：权限被拒绝",
        ErrorCode::CapabilityUnavailable => "Pinora - 截图未完成：当前环境不支持",
        ErrorCode::RetryablePlatform => "Pinora - 截图未完成：平台暂不可用",
        ErrorCode::TimedOut => "Pinora - 截图未完成：已超时",
        _ => "Pinora - 截图未完成，请重试",
    }
}

const fn delayed_capture_failed_label(error: ErrorCode) -> &'static str {
    match error {
        ErrorCode::PermissionDenied => "Pinora - 延时截图未完成：权限被拒绝",
        ErrorCode::CapabilityUnavailable => "Pinora - 延时截图未完成：当前环境不支持",
        ErrorCode::RetryablePlatform => "Pinora - 延时截图未完成：平台暂不可用",
        ErrorCode::TimedOut => "Pinora - 延时截图未完成：已超时",
        _ => "Pinora - 延时截图未完成，请重试",
    }
}

const fn ocr_failed_label(error: ErrorCode) -> &'static str {
    match error {
        ErrorCode::PermissionDenied => "Pinora - 文字识别未完成：权限被拒绝",
        ErrorCode::CapabilityUnavailable => "Pinora - 文字识别未完成：当前环境不支持",
        ErrorCode::TimedOut => "Pinora - 文字识别未完成：已超时",
        _ => "Pinora - 文字识别未完成，请重试",
    }
}

const fn export_running_label(operation: TrayExportOperation) -> &'static str {
    match operation {
        TrayExportOperation::CopyImage => "Pinora - 正在复制图像",
        TrayExportOperation::CopyText => "Pinora - 正在复制文字",
        TrayExportOperation::SaveFile => "Pinora - 正在保存文件",
    }
}

const fn export_completed_label(operation: TrayExportOperation) -> &'static str {
    match operation {
        TrayExportOperation::CopyImage => "Pinora - 图像已复制到系统剪贴板",
        TrayExportOperation::CopyText => "Pinora - 文字已复制到系统剪贴板",
        TrayExportOperation::SaveFile => "Pinora - 文件已保存",
    }
}

const fn export_failed_label(operation: TrayExportOperation, error: ErrorCode) -> &'static str {
    match (operation, error) {
        (TrayExportOperation::CopyImage, ErrorCode::ClipboardFailed) => {
            "Pinora - 图像复制未完成：系统剪贴板不可用"
        }
        (TrayExportOperation::CopyText, ErrorCode::ClipboardFailed) => {
            "Pinora - 文字复制未完成：系统剪贴板不可用"
        }
        (TrayExportOperation::CopyImage, ErrorCode::TimedOut) => "Pinora - 图像复制未完成：已超时",
        (TrayExportOperation::CopyText, ErrorCode::TimedOut) => "Pinora - 文字复制未完成：已超时",
        (TrayExportOperation::SaveFile, ErrorCode::TimedOut) => "Pinora - 文件保存未完成：已超时",
        (TrayExportOperation::CopyImage, _) => "Pinora - 图像复制未完成，请重试",
        (TrayExportOperation::CopyText, _) => "Pinora - 文字复制未完成，请重试",
        (TrayExportOperation::SaveFile, _) => "Pinora - 文件保存未完成，请重试",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_LABEL_CHARS: usize = 48;

    #[test]
    fn feedback_uses_only_static_sanitized_labels() {
        let feedback = [
            TrayFeedback::Ready,
            TrayFeedback::CapturePreparing,
            TrayFeedback::CaptureReady,
            TrayFeedback::CaptureCancelled,
            TrayFeedback::CaptureFailed(ErrorCode::PermissionDenied),
            TrayFeedback::CaptureFailed(ErrorCode::Internal),
            TrayFeedback::DelayedCaptureScheduled,
            TrayFeedback::DelayedCaptureCancelled,
            TrayFeedback::DelayedCaptureFailed(ErrorCode::TimedOut),
            TrayFeedback::OcrRunning,
            TrayFeedback::OcrCompleted,
            TrayFeedback::OcrFailed(ErrorCode::CapabilityUnavailable),
            TrayFeedback::ExportRunning(TrayExportOperation::CopyImage),
            TrayFeedback::ExportCompleted(TrayExportOperation::CopyText),
            TrayFeedback::ExportFailed(TrayExportOperation::SaveFile, ErrorCode::Internal),
            TrayFeedback::ExportFailed(TrayExportOperation::CopyImage, ErrorCode::ClipboardFailed),
        ];

        for status in feedback {
            let label = status.label();
            assert!(label.starts_with("Pinora - "));
            assert!(label.chars().count() <= MAX_LABEL_CHARS, "{label}");
            assert!(!label.contains('\n'));
            assert!(!label.contains('\r'));
            assert!(!label.contains("/home/"));
            assert!(!label.contains("permission_denied"));
        }
    }

    #[test]
    fn clipboard_failure_is_not_presented_as_success() {
        let label =
            TrayFeedback::ExportFailed(TrayExportOperation::CopyImage, ErrorCode::ClipboardFailed)
                .label();

        assert!(label.contains("未完成"));
        assert!(label.contains("系统剪贴板不可用"));
        assert!(!label.contains("已复制"));
    }

    #[test]
    fn diagnostic_data_exposes_only_fixed_operation_and_stable_error_code() {
        let failure =
            TrayFeedback::ExportFailed(TrayExportOperation::CopyImage, ErrorCode::ClipboardFailed);

        assert_eq!(failure.diagnostic_label(), "EXPORT FAILED");
        assert_eq!(failure.error_code(), Some(ErrorCode::ClipboardFailed));
        assert_eq!(TrayFeedback::OcrRunning.error_code(), None);
    }
}
