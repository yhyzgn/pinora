//! 历史加载会话的纯状态与结果门禁。
//!
//! 文件读取、worker、窗口和唯一 EventLoop 仍由 `desktop_shell` 持有。

use pinora_core::{AssetRef, HistoryEntry, JobId, JobOwner};

use crate::HistoryLoadPreparation;

/// 历史条目被消费的方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryLoadIntent {
    Preview,
    Reopen,
    Edit,
}

impl HistoryLoadIntent {
    pub const fn preparation(self) -> HistoryLoadPreparation {
        match self {
            Self::Preview => HistoryLoadPreparation::Preview,
            Self::Reopen => HistoryLoadPreparation::Pin,
            Self::Edit => HistoryLoadPreparation::Editor,
        }
    }
}

/// 已从历史面板快照的加载请求。
#[derive(Debug, Clone)]
pub struct HistoryLoadRequest {
    entry: HistoryEntry,
    intent: HistoryLoadIntent,
}

impl HistoryLoadRequest {
    pub const fn new(entry: HistoryEntry, intent: HistoryLoadIntent) -> Self {
        Self { entry, intent }
    }

    pub const fn entry(&self) -> &HistoryEntry {
        &self.entry
    }

    pub const fn intent(&self) -> HistoryLoadIntent {
        self.intent
    }

    pub const fn preparation(&self) -> HistoryLoadPreparation {
        self.intent.preparation()
    }

    pub fn asset(&self) -> AssetRef {
        AssetRef::new(self.entry.image_id, self.entry.generation)
    }

    pub fn matches_entry(&self, entry: &HistoryEntry) -> bool {
        entry.image_id == self.entry.image_id && entry.generation == self.entry.generation
    }

    pub fn into_entry(self) -> HistoryEntry {
        self.entry
    }
}

/// 已提交给历史读取服务的加载请求。
#[derive(Debug, Clone)]
pub struct ActiveHistoryLoad {
    job_id: JobId,
    request: HistoryLoadRequest,
}

impl ActiveHistoryLoad {
    pub const fn new(job_id: JobId, request: HistoryLoadRequest) -> Self {
        Self { job_id, request }
    }

    pub fn has_job_id(&self, job_id: JobId) -> bool {
        self.job_id == job_id
    }

    pub fn accepts_result(
        &self,
        selected: Option<&HistoryEntry>,
        job_id: JobId,
        owner: JobOwner,
    ) -> Option<AssetRef> {
        let selected = selected?;
        (self.has_job_id(job_id)
            && owner == JobOwner::History(self.request.entry.image_id)
            && self.request.matches_entry(selected))
        .then(|| self.request.asset())
    }

    pub fn into_request(self) -> HistoryLoadRequest {
        self.request
    }
}

#[cfg(test)]
mod tests {
    use super::{ActiveHistoryLoad, HistoryLoadIntent, HistoryLoadRequest};
    use crate::HistoryLoadPreparation;
    use pinora_core::{
        AssetGeneration, AssetRef, ContentDigest, DisplayId, HistoryEntry, HistoryEntrySpec,
        HistoryOcrState, ImageId, JobId, JobOwner, PixelRect,
    };

    fn entry(id: u64) -> HistoryEntry {
        HistoryEntry::new(HistoryEntrySpec {
            image_id: ImageId::from_raw(id),
            generation: AssetGeneration::INITIAL,
            created_at_ms: id,
            display: DisplayId::new("test-history"),
            source_rect: PixelRect::new(0, 0, 2, 2),
            file_name: format!("{id}.png"),
            byte_len: 1,
            digest: ContentDigest::of(b"history"),
            ocr: HistoryOcrState::Unknown,
        })
        .expect("history entry")
    }

    #[test]
    fn intent_maps_to_the_worker_preparation() {
        assert_eq!(
            HistoryLoadIntent::Preview.preparation(),
            HistoryLoadPreparation::Preview
        );
        assert_eq!(
            HistoryLoadIntent::Reopen.preparation(),
            HistoryLoadPreparation::Pin
        );
        assert_eq!(
            HistoryLoadIntent::Edit.preparation(),
            HistoryLoadPreparation::Editor
        );
    }

    #[test]
    fn request_matches_only_the_same_entry_generation() {
        let entry = entry(31);
        let request = HistoryLoadRequest::new(entry.clone(), HistoryLoadIntent::Preview);
        let changed = HistoryEntry {
            generation: entry.generation.advance().expect("advance generation"),
            ..entry.clone()
        };

        assert!(request.matches_entry(&entry));
        assert!(!request.matches_entry(&changed));
        assert_eq!(
            request.asset(),
            AssetRef::new(entry.image_id, entry.generation)
        );
    }

    #[test]
    fn active_load_accepts_only_the_current_selected_entry_and_owner() {
        let entry = entry(31);
        let active = ActiveHistoryLoad::new(
            JobId::from_raw(32),
            HistoryLoadRequest::new(entry.clone(), HistoryLoadIntent::Preview),
        );
        let asset = AssetRef::new(entry.image_id, entry.generation);

        assert_eq!(
            active.accepts_result(
                Some(&entry),
                JobId::from_raw(32),
                JobOwner::History(entry.image_id),
            ),
            Some(asset)
        );
        assert_eq!(
            active.accepts_result(
                Some(&entry),
                JobId::from_raw(33),
                JobOwner::History(entry.image_id),
            ),
            None
        );
        assert_eq!(
            active.accepts_result(
                Some(&entry),
                JobId::from_raw(32),
                JobOwner::History(ImageId::from_raw(99)),
            ),
            None
        );
        let changed = HistoryEntry {
            generation: entry.generation.advance().expect("advance generation"),
            ..entry
        };
        assert_eq!(
            active.accepts_result(
                Some(&changed),
                JobId::from_raw(32),
                JobOwner::History(changed.image_id),
            ),
            None
        );
    }
}
