//! 贴图实体与变换。

use crate::geometry::PixelPoint;
use crate::ids::{ImageId, PinId};
use crate::image::CaptureImage;

/// 贴图交互模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PinMode {
    /// 普通可交互。
    #[default]
    Interactive,
    /// 仅展示（为后续只读模式预留）。
    ViewOnly,
}

/// 贴图窗口变换（屏幕坐标，逻辑上独立于图像像素缓冲）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PinTransform {
    /// 贴图左上角屏幕位置。
    pub position: PixelPoint,
    /// 相对原始像素尺寸的缩放（1.0 = 100%）。
    pub scale: f64,
    /// 旋转角度（度，顺时针；Phase 0 仅存储）。
    pub rotation_deg: f64,
    /// 不透明度 0.0–1.0。
    pub opacity: f64,
}

impl PinTransform {
    pub fn default_at(position: PixelPoint) -> Self {
        Self {
            position,
            scale: 1.0,
            rotation_deg: 0.0,
            opacity: 1.0,
        }
    }

    pub fn clamped(mut self) -> Self {
        if !self.scale.is_finite() || self.scale <= 0.0 {
            self.scale = 1.0;
        }
        self.scale = self.scale.clamp(0.05, 8.0);
        if !self.opacity.is_finite() {
            self.opacity = 1.0;
        }
        self.opacity = self.opacity.clamp(0.05, 1.0);
        if !self.rotation_deg.is_finite() {
            self.rotation_deg = 0.0;
        }
        self
    }
}

/// 桌面贴图实体。
#[derive(Debug, Clone, PartialEq)]
pub struct Pin {
    pub id: PinId,
    pub image_id: ImageId,
    pub transform: PinTransform,
    pub mode: PinMode,
    pub locked: bool,
    pub always_on_top: bool,
}

impl Pin {
    pub fn from_capture(image: &CaptureImage, position: PixelPoint) -> Self {
        Self {
            id: PinId::new(),
            image_id: image.id,
            transform: PinTransform::default_at(position).clamped(),
            mode: PinMode::Interactive,
            locked: false,
            always_on_top: true,
        }
    }

    pub fn with_transform(mut self, transform: PinTransform) -> Self {
        self.transform = transform.clamped();
        self
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{PixelRect, PixelSize};
    use crate::image::{CaptureMetadata, DisplayId, RgbaBuffer};
    use crate::ids::ImageId;

    fn sample_image() -> CaptureImage {
        let pixels = RgbaBuffer::solid(PixelSize::new(20, 10), [0, 0, 0, 255]);
        CaptureImage::new(
            ImageId::new(),
            pixels,
            PixelRect::new(0, 0, 20, 10),
            CaptureMetadata::new(DisplayId::new("d0"), 1.0, 1),
        )
        .unwrap()
    }

    #[test]
    fn pin_from_capture_defaults() {
        let image = sample_image();
        let pin = Pin::from_capture(&image, PixelPoint::new(100, 200));
        assert_eq!(pin.image_id, image.id);
        assert_eq!(pin.transform.position, PixelPoint::new(100, 200));
        assert!(pin.always_on_top);
        assert!(!pin.locked);
        assert!((pin.transform.scale - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn transform_clamps_opacity_and_scale() {
        let t = PinTransform {
            position: PixelPoint::new(0, 0),
            scale: 100.0,
            rotation_deg: 0.0,
            opacity: 2.0,
        }
        .clamped();
        assert!((t.scale - 8.0).abs() < f64::EPSILON);
        assert!((t.opacity - 1.0).abs() < f64::EPSILON);
    }
}
