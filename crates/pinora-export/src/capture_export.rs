//! 已裁剪截图的导出来源选择与标注合成。

use pinora_core::{AnnotateSession, CaptureImage, bake_annotations, render_preview_rgba};

/// Overlay 复制与保存可选择的图像来源。
///
/// 贴图调用方仍可强制使用 [`Self::Annotated`]，避免将临时用户选择泄漏到贴图语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptureExportSource {
    Original,
    #[default]
    Annotated,
}

impl CaptureExportSource {
    pub const fn next(self) -> Self {
        match self {
            Self::Original => Self::Annotated,
            Self::Annotated => Self::Original,
        }
    }

    pub const fn bakes_annotations(self) -> bool {
        matches!(self, Self::Annotated)
    }
}

/// 根据冻结的来源生成单一 RGBA 导出图像。
///
/// 原图来源不读取标注文档或草稿。标注来源优先烧录已提交文档；没有已提交文档时，
/// 使用草稿预览。异常的预览长度绝不进入编码路径，而是稳定回退到裁剪原图。
pub fn compose_capture_export_image(
    crop: CaptureImage,
    annotate: &AnnotateSession,
    source: CaptureExportSource,
) -> CaptureImage {
    compose_with_preview(crop, annotate, source, render_preview_rgba)
}

fn compose_with_preview(
    crop: CaptureImage,
    annotate: &AnnotateSession,
    source: CaptureExportSource,
    render_preview: impl FnOnce(&CaptureImage, &AnnotateSession) -> Vec<u8>,
) -> CaptureImage {
    if !source.bakes_annotations() {
        return crop;
    }
    if !annotate.doc.is_empty() {
        return bake_annotations(&crop, &annotate.doc);
    }
    if annotate.draft.is_none() {
        return crop;
    }

    let rgba = render_preview(&crop, annotate);
    let mut image = crop;
    if rgba.len() == image.pixels.bytes.len() {
        image.pixels.bytes = rgba;
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{
        Annotation, CaptureMetadata, DisplayId, ImageId, PixelPoint, PixelRect, PixelSize,
        RgbaBuffer,
    };

    fn sample_image() -> CaptureImage {
        CaptureImage::new(
            ImageId::new(),
            RgbaBuffer::solid(PixelSize::new(8, 8), [12, 34, 56, 255]),
            PixelRect::new(0, 0, 8, 8),
            CaptureMetadata::new(DisplayId::new("export-source"), 1.0, 0),
        )
        .unwrap()
    }

    fn committed_session() -> AnnotateSession {
        let mut annotate = AnnotateSession::new(8, 8);
        annotate.doc.push(Annotation::Rect {
            a: PixelPoint::new(1, 1),
            b: PixelPoint::new(6, 6),
            color: [255, 0, 0, 255],
            stroke: 1,
            fill: None,
        });
        annotate
    }

    fn draft_session() -> AnnotateSession {
        let mut annotate = AnnotateSession::new(8, 8);
        annotate.begin(PixelPoint::new(1, 1));
        annotate.drag(PixelPoint::new(6, 6));
        assert!(annotate.doc.is_empty());
        assert!(annotate.draft.is_some());
        annotate
    }

    #[test]
    fn source_defaults_and_cycles() {
        let default = CaptureExportSource::default();

        assert_eq!(default, CaptureExportSource::Annotated);
        assert_eq!(default.next(), CaptureExportSource::Original);
        assert_eq!(default.next().next(), CaptureExportSource::Annotated);
    }

    #[test]
    fn original_source_does_not_render_annotations() {
        let image = sample_image();
        let annotate = committed_session();

        let output = compose_with_preview(
            image.clone(),
            &annotate,
            CaptureExportSource::Original,
            |_, _| panic!("original export must not render a preview"),
        );

        assert_eq!(output, image);
    }

    #[test]
    fn committed_document_is_baked_before_any_draft_preview() {
        let image = sample_image();
        let mut annotate = committed_session();
        annotate.begin(PixelPoint::new(2, 2));
        annotate.drag(PixelPoint::new(5, 5));
        assert!(annotate.draft.is_some());

        let output = compose_with_preview(
            image.clone(),
            &annotate,
            CaptureExportSource::Annotated,
            |_, _| panic!("committed document must win over preview rendering"),
        );

        assert_ne!(output.pixels.bytes, image.pixels.bytes);
    }

    #[test]
    fn draft_preview_is_used_only_when_the_document_is_empty() {
        let image = sample_image();
        let annotate = draft_session();
        let expected = render_preview_rgba(&image, &annotate);

        let output =
            compose_capture_export_image(image.clone(), &annotate, CaptureExportSource::Annotated);

        assert_eq!(output.pixels.bytes, expected);
        assert_ne!(output.pixels.bytes, image.pixels.bytes);
    }

    #[test]
    fn invalid_draft_preview_length_falls_back_to_the_cropped_image() {
        let image = sample_image();
        let annotate = draft_session();

        let output = compose_with_preview(
            image.clone(),
            &annotate,
            CaptureExportSource::Annotated,
            |_, _| vec![0; 3],
        );

        assert_eq!(output, image);
    }
}
