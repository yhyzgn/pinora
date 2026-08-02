//! 截图与像素缓冲的领域表示。

use crate::error::{ErrorCode, PinoraError};
use crate::geometry::{PixelRect, PixelSize};
use crate::ids::ImageId;
use crate::selection::clamp_to_image;

/// 显示器标识（平台无关字符串/序号占位）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DisplayId(pub String);

impl DisplayId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 由一次全虚拟桌面捕获产生的合成工作区来源。
    ///
    /// 此值不是平台显示器标识。工作区图像的坐标始终是物理像素，不能继承任一
    /// 显示器的逻辑缩放因子。
    pub fn virtual_desktop() -> Self {
        Self("pinora:virtual-desktop".into())
    }

    pub fn is_virtual_desktop(&self) -> bool {
        self.0 == "pinora:virtual-desktop"
    }
}

/// 捕获元数据（不含像素本体日志字段）。
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureMetadata {
    pub display: DisplayId,
    pub scale: f64,
    /// 捕获时刻 Unix 毫秒。
    pub captured_at_ms: u64,
}

impl CaptureMetadata {
    pub fn new(display: DisplayId, scale: f64, captured_at_ms: u64) -> Self {
        Self {
            display,
            scale: if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            },
            captured_at_ms,
        }
    }
}

/// RGBA8 像素缓冲（行优先，每像素 4 字节）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaBuffer {
    pub size: PixelSize,
    pub bytes: Vec<u8>,
}

impl RgbaBuffer {
    pub fn new(size: PixelSize, bytes: Vec<u8>) -> Result<Self, &'static str> {
        let expected = size.area().checked_mul(4).ok_or("buffer size overflow")?;
        if bytes.len() as u64 != expected {
            return Err("rgba byte length does not match size");
        }
        Ok(Self { size, bytes })
    }

    /// 创建指定尺寸的纯色缓冲（测试与占位用）。
    pub fn solid(size: PixelSize, rgba: [u8; 4]) -> Self {
        let mut bytes = Vec::with_capacity((size.area() * 4) as usize);
        for _ in 0..size.area() {
            bytes.extend_from_slice(&rgba);
        }
        Self { size, bytes }
    }

    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }
}

/// 一次截图产生的不可变像素与来源元数据。
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureImage {
    pub id: ImageId,
    pub pixels: RgbaBuffer,
    pub source_rect: PixelRect,
    pub metadata: CaptureMetadata,
}

impl CaptureImage {
    pub fn new(
        id: ImageId,
        pixels: RgbaBuffer,
        source_rect: PixelRect,
        metadata: CaptureMetadata,
    ) -> Result<Self, &'static str> {
        if pixels.size.is_empty() {
            return Err("capture image cannot be empty");
        }
        if source_rect.size != pixels.size {
            return Err("source_rect size must match pixel buffer");
        }
        Ok(Self {
            id,
            pixels,
            source_rect,
            metadata,
        })
    }

    pub fn size(&self) -> PixelSize {
        self.pixels.size
    }

    /// 按图像本地坐标裁剪（原点为缓冲左上角）。
    pub fn crop_local(&self, local_rect: PixelRect) -> Result<CaptureImage, PinoraError> {
        let rect = clamp_to_image(local_rect, self.pixels.size).ok_or_else(|| {
            PinoraError::new(ErrorCode::CommandRejected, "crop rect outside image")
        })?;
        if rect.size.is_empty() {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                "crop rect is empty",
            ));
        }

        let mut bytes = Vec::with_capacity((rect.size.area() * 4) as usize);
        let src_w = self.pixels.size.width as usize;
        for row in 0..rect.size.height as usize {
            let src_y = rect.origin.y as usize + row;
            let start = (src_y * src_w + rect.origin.x as usize) * 4;
            let end = start + rect.size.width as usize * 4;
            bytes.extend_from_slice(&self.pixels.bytes[start..end]);
        }
        let pixels = RgbaBuffer::new(rect.size, bytes)
            .map_err(|m| PinoraError::new(ErrorCode::Internal, m))?;

        let global = PixelRect::new(
            self.source_rect.origin.x + rect.origin.x,
            self.source_rect.origin.y + rect.origin.y,
            rect.size.width,
            rect.size.height,
        );
        CaptureImage::new(ImageId::new(), pixels, global, self.metadata.clone())
            .map_err(|m| PinoraError::new(ErrorCode::Internal, m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ImageId;

    #[test]
    fn rgba_buffer_rejects_mismatched_len() {
        let err = RgbaBuffer::new(PixelSize::new(2, 2), vec![0; 10]).unwrap_err();
        assert!(err.contains("byte length"));
    }

    #[test]
    fn capture_image_requires_matching_rect() {
        let pixels = RgbaBuffer::solid(PixelSize::new(10, 10), [255, 0, 0, 255]);
        let meta = CaptureMetadata::new(DisplayId::new("display-0"), 1.0, 0);
        let err = CaptureImage::new(ImageId::new(), pixels, PixelRect::new(0, 0, 5, 5), meta)
            .unwrap_err();
        assert!(err.contains("source_rect"));
    }

    #[test]
    fn solid_buffer_has_expected_len() {
        let buf = RgbaBuffer::solid(PixelSize::new(3, 2), [1, 2, 3, 4]);
        assert_eq!(buf.byte_len(), 3 * 2 * 4);
    }

    #[test]
    fn crop_local_extracts_sub_rect() {
        // 2x2 像素，各不相同
        let mut bytes = Vec::new();
        for v in [1u8, 2, 3, 4] {
            bytes.extend_from_slice(&[v, 0, 0, 255]);
        }
        let pixels = RgbaBuffer::new(PixelSize::new(2, 2), bytes).unwrap();
        let image = CaptureImage::new(
            ImageId::new(),
            pixels,
            PixelRect::new(100, 200, 2, 2),
            CaptureMetadata::new(DisplayId::new("d0"), 1.0, 0),
        )
        .unwrap();
        let cropped = image.crop_local(PixelRect::new(1, 0, 1, 1)).unwrap();
        assert_eq!(cropped.size(), PixelSize::new(1, 1));
        assert_eq!(cropped.pixels.bytes[0], 2);
        assert_eq!(cropped.source_rect, PixelRect::new(101, 200, 1, 1));
    }

    #[test]
    fn virtual_desktop_id_is_explicit_and_not_a_display_alias() {
        let id = DisplayId::virtual_desktop();
        assert!(id.is_virtual_desktop());
        assert!(!DisplayId::new("display-0").is_virtual_desktop());
    }
}
