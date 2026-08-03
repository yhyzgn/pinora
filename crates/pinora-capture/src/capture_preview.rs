//! Overlay 预览帧的无窗口像素转换与完整性契约。

use pinora_core::CaptureImage;

/// 可直接交给 Overlay 的预处理帧：原始图像、XRGB 基础帧与暗化底图。
///
/// 此值不持有窗口或图形表面；调用方必须在将其交给呈现层前检查
/// [`Self::matches_image`]，以拒绝不完整的跨模块缓冲。
pub struct CapturePreview {
    pub image: CaptureImage,
    pub base: Vec<u32>,
    pub dimmed: Vec<u32>,
}

impl CapturePreview {
    /// 从已验证的 RGBA 图像一次性生成 XRGB 基础帧与暗化底图。
    pub fn from_image(image: CaptureImage) -> Self {
        let (base, dimmed) = rgba_to_xrgb_and_dim(&image.pixels.bytes);
        Self {
            image,
            base,
            dimmed,
        }
    }

    /// 组装由已有预处理 worker 产出的帧；缓冲完整性通过 [`Self::matches_image`] 判断。
    pub fn from_parts(image: CaptureImage, base: Vec<u32>, dimmed: Vec<u32>) -> Self {
        Self {
            image,
            base,
            dimmed,
        }
    }

    /// 基础帧与暗化帧是否均严格覆盖图像的每个物理像素。
    pub fn matches_image(&self) -> bool {
        let Ok(expected_len) = usize::try_from(self.image.pixels.size.area()) else {
            return false;
        };
        self.base.len() == expected_len && self.dimmed.len() == expected_len
    }
}

/// 单遍：RGBA → XRGB。
pub fn rgba_to_xrgb(bytes: &[u8]) -> Vec<u32> {
    let n = bytes.len() / 4;
    let mut base = Vec::with_capacity(n);
    for c in bytes.chunks_exact(4) {
        let r = u32::from(c[0]);
        let g = u32::from(c[1]);
        let b = u32::from(c[2]);
        base.push((r << 16) | (g << 8) | b);
    }
    base
}

/// 单遍：RGBA → XRGB 基础帧和暗化帧。
pub fn rgba_to_xrgb_and_dim(bytes: &[u8]) -> (Vec<u32>, Vec<u32>) {
    let n = bytes.len() / 4;
    let mut base = Vec::with_capacity(n);
    let mut dimmed = Vec::with_capacity(n);
    for c in bytes.chunks_exact(4) {
        let r = u32::from(c[0]);
        let g = u32::from(c[1]);
        let b = u32::from(c[2]);
        base.push((r << 16) | (g << 8) | b);
        // ≈55% 亮度
        let dr = r * 11 / 20;
        let dg = g * 11 / 20;
        let db = b * 11 / 20;
        dimmed.push((dr << 16) | (dg << 8) | db);
    }
    (base, dimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{CaptureMetadata, DisplayId, ImageId, PixelRect, PixelSize, RgbaBuffer};

    fn sample_image() -> CaptureImage {
        CaptureImage::new(
            ImageId::new(),
            RgbaBuffer::new(PixelSize::new(2, 1), vec![1, 2, 3, 255, 4, 5, 6, 255]).unwrap(),
            PixelRect::new(0, 0, 2, 1),
            CaptureMetadata::new(DisplayId::new("test-display"), 1.0, 0),
        )
        .unwrap()
    }

    #[test]
    fn preview_conversion_matches_its_image_and_rejects_short_buffers() {
        let mut preview = CapturePreview::from_image(sample_image());

        assert_eq!(preview.base, vec![0x0001_0203, 0x0004_0506]);
        assert!(preview.matches_image());

        preview.dimmed.pop();
        assert!(!preview.matches_image());
    }

    #[test]
    fn preview_from_parts_keeps_worker_pixels_without_reconversion() {
        let preview = CapturePreview::from_parts(
            sample_image(),
            vec![0x0011_2233, 0x0044_5566],
            vec![0x0009_0807, 0x0006_0504],
        );

        assert_eq!(preview.base, vec![0x0011_2233, 0x0044_5566]);
        assert_eq!(preview.dimmed, vec![0x0009_0807, 0x0006_0504]);
        assert!(preview.matches_image());
    }

    #[test]
    fn rgba_conversion_preserves_pixels_and_creates_a_dimmed_frame() {
        let bytes = vec![255u8, 0, 0, 255, 0, 255, 0, 255];
        let (base, dimmed) = rgba_to_xrgb_and_dim(&bytes);

        assert_eq!(base, vec![0x00ff_0000, 0x0000_ff00]);
        assert_eq!(dimmed.len(), 2);
        assert_eq!(rgba_to_xrgb(&bytes), base);
    }
}
