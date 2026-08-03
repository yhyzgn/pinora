//! 导出请求的纯状态和值对象。
//!
//! 本模块只描述 Overlay 完成后的导出意图、像素来源、动作分类和提交前冻结的输出目标。
//! 文件名分配、作业提交、文件/剪贴板 IO、历史登记、tray 与窗口生命周期由调用方编排。

use std::path::PathBuf;

use pinora_core::ExportImageFormat;

use crate::CaptureExportSource;

/// Overlay 确认选区后的用户动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayExportAction {
    Copy,
    Pin,
    Save,
}

/// 根据 Overlay 完成动作选择应导出的图像来源。
///
/// 新贴图必须持有标注后的像素，不能因为用户导出偏好选择原图；复制和保存继续使用会话选择。
pub const fn capture_export_source_for_overlay_action(
    action: OverlayExportAction,
    selected: CaptureExportSource,
) -> CaptureExportSource {
    match action {
        OverlayExportAction::Copy | OverlayExportAction::Save => selected,
        OverlayExportAction::Pin => CaptureExportSource::Annotated,
    }
}

/// 已提交给导出服务的动作及其冻结参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportAction {
    SaveImage(PathBuf),
    CopyImage,
    CopyText,
}

impl ExportAction {
    pub const fn operation(&self) -> ExportOperation {
        match self {
            Self::SaveImage(_) => ExportOperation::SaveImage,
            Self::CopyImage => ExportOperation::CopyImage,
            Self::CopyText => ExportOperation::CopyText,
        }
    }

    pub const fn is_file_save(&self) -> bool {
        matches!(self, Self::SaveImage(_))
    }
}

/// 与 UI 无关的导出动作分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportOperation {
    SaveImage,
    CopyImage,
    CopyText,
}

/// 保存任务提交前冻结的输出参数。
///
/// 导出 worker 绝不读取随后变化的运行时设置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenExportTarget {
    pub path: PathBuf,
    pub format: ExportImageFormat,
    pub jpeg_quality: u8,
}

impl FrozenExportTarget {
    pub fn new(path: PathBuf, format: ExportImageFormat, jpeg_quality: u8) -> Self {
        Self {
            path,
            format,
            jpeg_quality,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_action_always_uses_the_annotated_export_source() {
        assert_eq!(
            capture_export_source_for_overlay_action(
                OverlayExportAction::Copy,
                CaptureExportSource::Original,
            ),
            CaptureExportSource::Original
        );
        assert_eq!(
            capture_export_source_for_overlay_action(
                OverlayExportAction::Save,
                CaptureExportSource::Original,
            ),
            CaptureExportSource::Original
        );
        assert_eq!(
            capture_export_source_for_overlay_action(
                OverlayExportAction::Pin,
                CaptureExportSource::Original,
            ),
            CaptureExportSource::Annotated
        );
    }

    #[test]
    fn actions_have_stable_operations_and_file_save_scope() {
        assert_eq!(
            ExportAction::CopyImage.operation(),
            ExportOperation::CopyImage
        );
        assert_eq!(
            ExportAction::CopyText.operation(),
            ExportOperation::CopyText
        );
        let save = ExportAction::SaveImage(PathBuf::from("export.png"));
        assert_eq!(save.operation(), ExportOperation::SaveImage);
        assert!(save.is_file_save());
        assert!(!ExportAction::CopyImage.is_file_save());
        assert!(!ExportAction::CopyText.is_file_save());
    }

    #[test]
    fn frozen_export_target_keeps_the_submission_parameters() {
        let target =
            FrozenExportTarget::new(PathBuf::from("capture.webp"), ExportImageFormat::WebP, 87);

        assert_eq!(target.path, PathBuf::from("capture.webp"));
        assert_eq!(target.format, ExportImageFormat::WebP);
        assert_eq!(target.jpeg_quality, 87);
    }
}
