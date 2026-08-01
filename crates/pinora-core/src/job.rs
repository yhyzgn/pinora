//! 受监督耗时任务的领域身份与终态。
//!
//! 本模块不持有线程、子进程或窗口句柄。应用层监督器依据这些不可变值对象
//! 管理取消与结果提交，平台适配器则负责回收自己创建的实际工作单元。

use crate::asset::AssetRef;
use crate::ids::{CorrelationId, JobId, PinId, SessionId};

/// 任务所属的领域实体；不包含窗口或线程句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobOwner {
    Session(SessionId),
    Pin(PinId),
}

/// 任务的业务类别，用于后续按类别配置并发与超时策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobKind {
    Capture,
    Ocr,
    Export,
    Clipboard,
}

/// 提交任务时冻结的不可变元数据。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobSpec {
    pub id: JobId,
    pub correlation_id: CorrelationId,
    pub asset: AssetRef,
    pub owner: JobOwner,
    pub kind: JobKind,
    /// 单调时钟或测试时钟上的截止时间（毫秒）。`now >= deadline_at_ms` 即超时。
    pub deadline_at_ms: u64,
}

impl JobSpec {
    pub const fn new(
        id: JobId,
        correlation_id: CorrelationId,
        asset: AssetRef,
        owner: JobOwner,
        kind: JobKind,
        deadline_at_ms: u64,
    ) -> Self {
        Self {
            id,
            correlation_id,
            asset,
            owner,
            kind,
            deadline_at_ms,
        }
    }

    pub const fn is_expired_at(self, now_ms: u64) -> bool {
        now_ms >= self.deadline_at_ms
    }
}

/// 工作单元返回结果时必须一并带回的身份；不包含 OCR 文本或图像像素。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobResultRef {
    pub job_id: JobId,
    pub asset: AssetRef,
}

impl JobResultRef {
    pub const fn new(job_id: JobId, asset: AssetRef) -> Self {
        Self { job_id, asset }
    }
}

/// 任务不可再接受结果的终态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobTerminalState {
    Completed,
    Failed,
    Cancelled,
    OwnerClosed,
    TimedOut,
    StaleAsset,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AssetGeneration, ImageId};

    #[test]
    fn spec_freezes_identity_owner_asset_and_deadline() {
        let asset = AssetRef::new(
            ImageId::from_raw(11),
            AssetGeneration::from_raw(4).expect("valid generation"),
        );
        let spec = JobSpec::new(
            JobId::from_raw(5),
            CorrelationId::from_raw(6),
            asset,
            JobOwner::Pin(PinId::from_raw(7)),
            JobKind::Ocr,
            100,
        );

        assert_eq!(spec.asset, asset);
        assert_eq!(spec.owner, JobOwner::Pin(PinId::from_raw(7)));
        assert!(!spec.is_expired_at(99));
        assert!(spec.is_expired_at(100));
        assert_eq!(JobResultRef::new(spec.id, asset).job_id, spec.id);
    }
}
