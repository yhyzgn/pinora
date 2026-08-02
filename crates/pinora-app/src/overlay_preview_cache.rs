//! Overlay 单会话标注预览缓存。
//!
//! 已提交文档只在源选区或 revision 改变时烧录一次；正在拖拽的草稿随后从该层复制并
//! 叠加。缓存绝不跨 Overlay 会话保存，马赛克和模糊仍由 core 从不可变源裁剪取样。

use pinora_core::{
    AnnotateSession, AnnotationRevision, CaptureImage, PixelRect, bake_annotations,
    render_draft_rgba,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CacheKey {
    source_rect: PixelRect,
    revision: AnnotationRevision,
}

#[derive(Default)]
pub(crate) struct OverlayPreviewCache {
    key: Option<CacheKey>,
    source_crop: Option<CaptureImage>,
    committed_rgba: Vec<u8>,
    #[cfg(test)]
    rebuild_count: usize,
}

impl OverlayPreviewCache {
    pub(crate) fn clear(&mut self) {
        self.key = None;
        self.source_crop = None;
        self.committed_rgba.clear();
    }

    /// 合成当前草稿，返回独立的预览缓冲以供调用方转换为显示像素。
    pub(crate) fn compose(
        &mut self,
        full_image: &CaptureImage,
        source_rect: PixelRect,
        session: &AnnotateSession,
    ) -> Option<Vec<u8>> {
        let key = CacheKey {
            source_rect,
            revision: session.doc.revision(),
        };
        if self.key != Some(key) {
            self.clear();
            let crop = full_image.crop_local(source_rect).ok()?;
            self.committed_rgba = bake_annotations(&crop, &session.doc).pixels.bytes;
            self.source_crop = Some(crop);
            self.key = Some(key);
            #[cfg(test)]
            {
                self.rebuild_count += 1;
            }
        }

        let mut preview = self.committed_rgba.clone();
        let source = self.source_crop.as_ref()?;
        let _ = render_draft_rgba(source, session, &mut preview);
        Some(preview)
    }

    #[cfg(test)]
    fn rebuild_count(&self) -> usize {
        self.rebuild_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{
        Annotation, CaptureMetadata, DisplayId, ImageId, PixelPoint, PixelSize, RgbaBuffer,
        render_preview_rgba,
    };

    fn source(width: u32, height: u32) -> CaptureImage {
        let mut bytes = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                bytes.extend_from_slice(&[
                    (x * 11) as u8,
                    (y * 17) as u8,
                    ((x + y) * 7) as u8,
                    255,
                ]);
            }
        }
        CaptureImage::new(
            ImageId::new(),
            RgbaBuffer::new(PixelSize::new(width, height), bytes).unwrap(),
            PixelRect::new(0, 0, width, height),
            CaptureMetadata::new(DisplayId::new("test"), 1.0, 0),
        )
        .unwrap()
    }

    #[test]
    fn reuses_committed_layer_across_draft_movements_and_rebuilds_on_revision_or_rect() {
        let image = source(40, 30);
        let first_rect = PixelRect::new(2, 3, 24, 18);
        let second_rect = PixelRect::new(4, 2, 20, 16);
        let mut session = AnnotateSession::new(24, 18);
        session.doc.push(Annotation::Mosaic {
            a: PixelPoint::new(3, 3),
            b: PixelPoint::new(10, 10),
            block: 4,
        });
        session.tool = pinora_core::AnnotateTool::Blur;
        session.begin(PixelPoint::new(12, 4));
        session.drag(PixelPoint::new(20, 12));

        let mut cache = OverlayPreviewCache::default();
        let first_crop = image.crop_local(first_rect).unwrap();
        assert_eq!(
            cache.compose(&image, first_rect, &session),
            Some(render_preview_rgba(&first_crop, &session))
        );
        assert_eq!(cache.rebuild_count(), 1);

        session.drag(PixelPoint::new(21, 13));
        assert_eq!(
            cache.compose(&image, first_rect, &session),
            Some(render_preview_rgba(&first_crop, &session))
        );
        assert_eq!(cache.rebuild_count(), 1);

        session.commit();
        assert_eq!(
            cache.compose(&image, first_rect, &session),
            Some(render_preview_rgba(&first_crop, &session))
        );
        assert_eq!(cache.rebuild_count(), 2);

        let second_crop = image.crop_local(second_rect).unwrap();
        assert_eq!(
            cache.compose(&image, second_rect, &session),
            Some(render_preview_rgba(&second_crop, &session))
        );
        assert_eq!(cache.rebuild_count(), 3);
    }

    #[test]
    fn failed_crop_clears_prior_cache_instead_of_reusing_old_pixels() {
        let image = source(12, 10);
        let rect = PixelRect::new(0, 0, 8, 8);
        let mut session = AnnotateSession::new(8, 8);
        session.doc.push(Annotation::Blur {
            a: PixelPoint::new(1, 1),
            b: PixelPoint::new(6, 6),
            radius: 4,
        });
        let mut cache = OverlayPreviewCache::default();
        assert!(cache.compose(&image, rect, &session).is_some());
        assert_eq!(cache.rebuild_count(), 1);

        assert_eq!(
            cache.compose(&image, PixelRect::new(20, 20, 8, 8), &session),
            None
        );
        assert!(cache.compose(&image, rect, &session).is_some());
        assert_eq!(cache.rebuild_count(), 2);
    }
}
