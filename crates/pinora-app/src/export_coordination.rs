//! 导出结果的 app 协调状态。
//!
//! 此模块持有与历史、任务状态和 tray 反馈耦合的待处理作业元数据。导出意图、动作分类和
//! 冻结输出目标由 `pinora-export::export_contract` 唯一拥有；文件名分配、作业提交、IO 和窗口
//! 生命周期继续由 `desktop_shell` 编排。

use std::collections::HashMap;

use pinora_core::{AssetRef, JobId, JobOwner};
use pinora_desktop::tray_feedback::TrayExportOperation;
use pinora_export::{ExportAction, ExportOperation};
use pinora_history::HistoryExportCandidate;
use pinora_jobs::JobState;

#[derive(Debug)]
pub(crate) struct PendingExport {
    pub(crate) owner: JobOwner,
    pub(crate) asset: AssetRef,
    pub(crate) action: ExportAction,
    pub(crate) history: Option<HistoryExportCandidate>,
}

impl PendingExport {
    pub(crate) fn new(
        owner: JobOwner,
        asset: AssetRef,
        action: ExportAction,
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

pub(crate) fn pending_asset_for_owner(
    pending_assets: &HashMap<JobId, (JobOwner, AssetRef)>,
    job_id: JobId,
    owner: JobOwner,
) -> Option<AssetRef> {
    pending_assets
        .get(&job_id)
        .and_then(|(pending_owner, asset)| (*pending_owner == owner).then_some(*asset))
}

pub(crate) const fn tray_export_operation(action: &ExportAction) -> TrayExportOperation {
    match action.operation() {
        ExportOperation::SaveImage => TrayExportOperation::SaveFile,
        ExportOperation::CopyImage => TrayExportOperation::CopyImage,
        ExportOperation::CopyText => TrayExportOperation::CopyText,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{ImageId, JobTerminalState, SessionId};
    use std::path::PathBuf;

    #[test]
    fn pending_actions_map_to_their_stable_tray_operations() {
        assert_eq!(
            tray_export_operation(&ExportAction::CopyImage),
            TrayExportOperation::CopyImage
        );
        assert_eq!(
            tray_export_operation(&ExportAction::CopyText),
            TrayExportOperation::CopyText
        );
        assert_eq!(
            tray_export_operation(&ExportAction::SaveImage(PathBuf::from("export.png"))),
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
                ExportAction::SaveImage(PathBuf::from("a.png")),
                None,
            ),
        );
        pending.insert(
            JobId::from_raw(2),
            PendingExport::new(owner, asset, ExportAction::CopyImage, None),
        );
        pending.insert(
            JobId::from_raw(3),
            PendingExport::new(owner, asset, ExportAction::CopyText, None),
        );
        pending.insert(
            JobId::from_raw(4),
            PendingExport::new(
                owner,
                asset,
                ExportAction::SaveImage(PathBuf::from("b.png")),
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
}
