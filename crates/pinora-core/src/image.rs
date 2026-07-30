//! 截图与像素缓冲的领域表示。

use crate::geometry::{PixelRect, PixelSize};
use crate::ids::ImageId;

/// 显示器标识（平台无关字符串/序号占位）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DisplayId(pub String);

impl DisplayId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
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
        let expected = size
            .area()
            .checked_mul(4)
            .ok_or("buffer size overflow")?;
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
        let err = CaptureImage::new(
            ImageId::new(),
            pixels,
            PixelRect::new(0, 0, 5, 5),
            meta,
        )
        .unwrap_err();
        assert!(err.contains("source_rect"));
    }

    #[test]
    fn solid_buffer_has_expected_len() {
        let buf = RgbaBuffer::solid(PixelSize::new(3, 2), [1, 2, 3, 4]);
        assert_eq!(buf.byte_len(), 3 * 2 * 4);
    }
}
