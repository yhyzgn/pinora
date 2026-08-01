//! 离线假截图实现：固定虚拟显示器 + 纯色区域像素。

use std::time::{SystemTime, UNIX_EPOCH};

use pinora_core::{
    CaptureImage, CaptureMetadata, CaptureProvider, CaptureRequest, DisplayId, DisplayInfo,
    ImageId, PinoraError, PixelRect, RgbaBuffer, resolve_capture_rect,
};

/// 提供单个 1920×1080 虚拟显示器，区域捕获返回纯色 RGBA。
#[derive(Debug, Clone)]
pub struct FakeCaptureProvider {
    displays: Vec<DisplayInfo>,
    /// 填充色 RGBA。
    fill: [u8; 4],
}

impl FakeCaptureProvider {
    pub fn new() -> Self {
        Self {
            displays: vec![DisplayInfo {
                id: DisplayId::new("fake-0"),
                name: "Fake Primary".into(),
                bounds: PixelRect::new(0, 0, 1920, 1080),
                scale: 1.0,
            }],
            fill: [0x2d, 0x6a, 0x4f, 0xff],
        }
    }

    pub fn with_fill(mut self, fill: [u8; 4]) -> Self {
        self.fill = fill;
        self
    }

    pub fn primary_display_id(&self) -> DisplayId {
        self.displays[0].id.clone()
    }
}

impl Default for FakeCaptureProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureProvider for FakeCaptureProvider {
    fn displays(&self) -> Result<Vec<DisplayInfo>, PinoraError> {
        Ok(self.displays.clone())
    }

    fn capture(&self, request: CaptureRequest) -> Result<CaptureImage, PinoraError> {
        let (info, rect) = resolve_capture_rect(&self.displays, &request)?;
        let pixels = RgbaBuffer::solid(rect.size, self.fill);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        CaptureImage::new(
            ImageId::new(),
            pixels,
            rect,
            CaptureMetadata::new(info.id, info.scale, now_ms),
        )
        .map_err(|msg| PinoraError::new(pinora_core::ErrorCode::Internal, msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::PixelSize;

    #[test]
    fn captures_region_size() {
        let provider = FakeCaptureProvider::new();
        let image = provider
            .capture(CaptureRequest::Region {
                display: provider.primary_display_id(),
                rect: PixelRect::new(10, 20, 100, 50),
            })
            .unwrap();
        assert_eq!(image.size(), PixelSize::new(100, 50));
        assert_eq!(image.pixels.byte_len(), 100 * 50 * 4);
    }

    #[test]
    fn full_display_matches_bounds() {
        let provider = FakeCaptureProvider::new();
        let image = provider
            .capture(CaptureRequest::FullDisplay {
                display: provider.primary_display_id(),
            })
            .unwrap();
        assert_eq!(image.size(), PixelSize::new(1920, 1080));
    }
}
