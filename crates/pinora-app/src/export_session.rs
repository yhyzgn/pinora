//! 导出会话的纯状态和值对象。
//!
//! 本模块定义 Overlay 完成意图、待处理导出元数据和结果判定。运行时读取、文件名
//! 分配、任务提交、文件/剪贴板 IO、tray 调用和窗口生命周期继续由 `desktop_shell` 编排。

use std::collections::HashMap;
use std::path::PathBuf;

use pinora_core::{AssetRef, ExportImageFormat, JobId, JobOwner};
use pinora_desktop::tray_feedback::TrayExportOperation;
use pinora_export::CaptureExportSource;
use pinora_history::HistoryExportCandidate;
use pinora_jobs::JobState;

/// 选区完成后的收尾动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayFinish {
    Copy,
    Pin,
    Save,
}

pub(crate) const fn export_source_for_overlay_finish(
    action: OverlayFinish,
    selected: CaptureExportSource,
) -> CaptureExportSource {
    match action {
        OverlayFinish::Copy | OverlayFinish::Save => selected,
        OverlayFinish::Pin => CaptureExportSource::Annotated,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PendingExportAction {
    SaveImage(PathBuf),
    CopyImage,
    CopyText,
}

impl PendingExportAction {
    const fn is_file_save(&self) -> bool {
        matches!(self, Self::SaveImage(_))
    }

    pub(crate) const fn tray_operation(&self) -> TrayExportOperation {
        match self {
            Self::SaveImage(_) => TrayExportOperation::SaveFile,
            Self::CopyImage => TrayExportOperation::CopyImage,
            Self::CopyText => TrayExportOperation::CopyText,
        }
    }
}

pub(crate) const fn tray_export_operation(action: &PendingExportAction) -> TrayExportOperation {
    action.tray_operation()
}

/// tray 只能取消仍由导出监督器标记为运行中的文件保存。待收敛的终态和所有
/// clipboard 任务继续留在 pending 映射中，直到 worker 结果被消费。
pub(crate) fn running_file_export_ids<F>(
    pending_exports: &HashMap<JobId, PendingExport>,
    mut state: F,
) -> Vec<JobId>
where
    F: FnMut(JobId) -> Option<JobState>,
{
    pending_exports
        .iter()
        .filter_map(|(job_id, pending)| {
            (pending.action.is_file_save() && matches!(state(*job_id), Some(JobState::Running)))
                .then_some(*job_id)
        })
        .collect()
}

/// 保存任务提交前冻结的输出参数。worker 绝不读取随后变化的运行时设置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrozenExportTarget {
    pub(crate) path: PathBuf,
    pub(crate) format: ExportImageFormat,
    pub(crate) jpeg_quality: u8,
}

impl FrozenExportTarget {
    pub(crate) fn new(path: PathBuf, format: ExportImageFormat, jpeg_quality: u8) -> Self {
        Self {
            path,
            format,
            jpeg_quality,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PendingExport {
    pub(crate) owner: JobOwner,
    pub(crate) asset: AssetRef,
    pub(crate) action: PendingExportAction,
    pub(crate) history: Option<HistoryExportCandidate>,
}

impl PendingExport {
    pub(crate) fn new(
        owner: JobOwner,
        asset: AssetRef,
        action: PendingExportAction,
        history: Option<HistoryExportCandidate>,
    ) -> Self {
        Self {
            owner,
            asset,
            action,
            history,
        }
    }
}

pub(crate) fn pending_asset_for_owner(
    pending_assets: &HashMap<JobId, (JobOwner, AssetRef)>,
    job_id: JobId,
    owner: JobOwner,
) -> Option<AssetRef> {
    pending_assets
        .get(&job_id)
        .and_then(|(pending_owner, asset)| (*pending_owner == owner).then_some(*asset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{ImageId, JobTerminalState, SessionId};

    #[test]
    fn pin_finish_always_uses_the_annotated_export_source() {
        assert_eq!(
            export_source_for_overlay_finish(OverlayFinish::Copy, CaptureExportSource::Original),
            CaptureExportSource::Original
        );
        assert_eq!(
            export_source_for_overlay_finish(OverlayFinish::Save, CaptureExportSource::Original),
            CaptureExportSource::Original
        );
        assert_eq!(
            export_source_for_overlay_finish(OverlayFinish::Pin, CaptureExportSource::Original),
            CaptureExportSource::Annotated
        );
    }

    #[test]
    fn pending_actions_map_to_their_stable_tray_operations() {
        assert_eq!(
            tray_export_operation(&PendingExportAction::CopyImage),
            TrayExportOperation::CopyImage
        );
        assert_eq!(
            tray_export_operation(&PendingExportAction::CopyText),
            TrayExportOperation::CopyText
        );
        assert_eq!(
            tray_export_operation(&PendingExportAction::SaveImage(PathBuf::from("export.png"))),
            TrayExportOperation::SaveFile
        );
    }

    #[test]
    fn pending_export_asset_requires_matching_owner() {
        let job_id = JobId::from_raw(7);
        let owner = JobOwner::Session(SessionId::from_raw(8));
        let asset = AssetRef::initial(ImageId::from_raw(9));
        let mut pending = HashMap::new();
        pending.insert(job_id, (owner, asset));

        assert_eq!(
            pending_asset_for_owner(&pending, job_id, owner),
            Some(asset)
        );
        assert_eq!(
            pending_asset_for_owner(&pending, job_id, JobOwner::Session(SessionId::from_raw(10))),
            None
        );
    }

    #[test]
    fn file_export_cancellation_selects_only_running_save_jobs() {
        let asset = AssetRef::initial(ImageId::from_raw(93));
        let owner = JobOwner::Session(SessionId::from_raw(93));
        let mut pending = HashMap::new();
        pending.insert(
            JobId::from_raw(1),
            PendingExport::new(
                owner,
                asset,
                PendingExportAction::SaveImage(PathBuf::from("a.png")),
                None,
            ),
        );
        pending.insert(
            JobId::from_raw(2),
            PendingExport::new(owner, asset, PendingExportAction::CopyImage, None),
        );
        pending.insert(
            JobId::from_raw(3),
            PendingExport::new(owner, asset, PendingExportAction::CopyText, None),
        );
        pending.insert(
            JobId::from_raw(4),
            PendingExport::new(
                owner,
                asset,
                PendingExportAction::SaveImage(PathBuf::from("b.png")),
                None,
            ),
        );

        let selected = running_file_export_ids(&pending, |job_id| match job_id.raw() {
            1..=3 => Some(JobState::Running),
            4 => Some(JobState::Finished(JobTerminalState::Cancelled)),
            _ => None,
        });

        assert_eq!(selected, vec![JobId::from_raw(1)]);
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
