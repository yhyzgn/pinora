//! Overlay 会话的纯状态与派生资产身份。
//!
//! Overlay 窗口、绘制、输入、标注文档写入和任务提交仍由 `desktop_shell` 持有。

use pinora_core::{AnnotationRevision, AssetGeneration, AssetRef, CaptureImage, ImageId};

/// Overlay 内阶段：框选中 / 已出选区（工具栏就绪）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayPhase {
    Selecting,
    Ready,
}

/// 已确认 Overlay 选区的派生图像身份。
///
/// 选区内标注只改变 generation；重选来源像素时才生成新的图像身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OverlayAssetIdentity {
    image_id: ImageId,
}

impl OverlayAssetIdentity {
    pub(crate) fn new() -> Self {
        Self {
            image_id: ImageId::new(),
        }
    }

    pub(crate) fn current(self, revision: AnnotationRevision) -> AssetRef {
        let generation = AssetGeneration::from_raw(revision.raw())
            .expect("annotation revision is guaranteed non-zero");
        AssetRef::new(self.image_id, generation)
    }

    pub(crate) fn stamp(self, image: &mut CaptureImage) {
        image.id = self.image_id;
    }
}

pub(crate) fn overlay_asset_for_revision(
    identity: Option<OverlayAssetIdentity>,
    revision: AnnotationRevision,
) -> Option<AssetRef> {
    identity.map(|identity| identity.current(revision))
}

#[cfg(test)]
mod tests {
    use super::OverlayAssetIdentity;
    use pinora_core::{
        Annotation, AnnotationDoc, AnnotationRevision, CaptureImage, CaptureMetadata,
        CorrelationId, DEFAULT_STROKE, DEFAULT_WIDTH, DisplayId, ImageId, JobId, JobKind, JobOwner,
        JobResultRef, PixelPoint, PixelRect, RgbaBuffer, SessionId,
    };
    use pinora_jobs::{JobResultDisposition, JobSupervisor};

    #[test]
    fn annotation_revision_changes_overlay_asset_and_rejects_late_result() {
        let identity = OverlayAssetIdentity::new();
        let mut doc = AnnotationDoc::new();
        let submitted = identity.current(doc.revision());

        doc.push(Annotation::Rect {
            a: PixelPoint::new(1, 1),
            b: PixelPoint::new(4, 4),
            color: DEFAULT_STROKE,
            stroke: DEFAULT_WIDTH,
            fill: None,
        });
        let current = identity.current(doc.revision());
        assert_eq!(submitted.image_id, current.image_id);
        assert_ne!(submitted.generation, current.generation);

        let spec = pinora_core::JobSpec::new(
            JobId::from_raw(11),
            CorrelationId::from_raw(12),
            submitted,
            JobOwner::Session(SessionId::from_raw(13)),
            JobKind::Ocr,
            100,
        );
        let mut supervisor = JobSupervisor::new();
        let ticket = supervisor.submit(spec).expect("submit overlay OCR");
        assert_eq!(
            supervisor
                .accept_result(JobResultRef::new(ticket.id, submitted), current, 1)
                .expect("known job"),
            JobResultDisposition::Rejected(pinora_core::JobTerminalState::StaleAsset)
        );

        let before_empty_undo = identity.current(doc.revision());
        assert!(doc.undo().is_some());
        assert_ne!(identity.current(doc.revision()), before_empty_undo);
        let after_undo = identity.current(doc.revision());
        assert_eq!(doc.undo(), None);
        assert_eq!(identity.current(doc.revision()), after_undo);
    }

    #[test]
    fn redo_produces_a_fresh_overlay_asset_generation() {
        let identity = OverlayAssetIdentity::new();
        let mut doc = AnnotationDoc::new();
        doc.push(Annotation::Rect {
            a: PixelPoint::new(1, 1),
            b: PixelPoint::new(4, 4),
            color: DEFAULT_STROKE,
            stroke: DEFAULT_WIDTH,
            fill: None,
        });
        let committed = identity.current(doc.revision());
        assert!(doc.undo().is_some());
        let undone = identity.current(doc.revision());
        assert!(doc.redo().is_some());
        let redone = identity.current(doc.revision());

        assert_eq!(committed.image_id, undone.image_id);
        assert_eq!(undone.image_id, redone.image_id);
        assert_ne!(committed.generation, undone.generation);
        assert_ne!(undone.generation, redone.generation);
        assert_ne!(committed.generation, redone.generation);
    }

    #[test]
    fn reselection_uses_a_new_image_identity_and_stamps_derived_image() {
        let first = OverlayAssetIdentity::new();
        let second = OverlayAssetIdentity::new();
        let revision = AnnotationRevision::INITIAL;
        assert_ne!(
            first.current(revision).image_id,
            second.current(revision).image_id
        );
        assert_eq!(
            first.current(revision).generation,
            second.current(revision).generation
        );

        let mut image = CaptureImage::new(
            ImageId::from_raw(99),
            RgbaBuffer::solid(pinora_core::PixelSize::new(2, 2), [1, 2, 3, 255]),
            PixelRect::new(0, 0, 2, 2),
            CaptureMetadata::new(DisplayId::new("test"), 1.0, 0),
        )
        .expect("derived test image");
        first.stamp(&mut image);
        assert_eq!(image.id, first.current(revision).image_id);
    }
}
