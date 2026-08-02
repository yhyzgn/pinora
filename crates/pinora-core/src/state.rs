use crate::error::{ErrorCode, PinoraError};
use crate::geometry::PixelPoint;
use crate::ids::{ImageId, PinId};
use crate::image::CaptureImage;
use crate::pin::{Pin, PinTransform};

/// 应用生命周期阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppPhase {
    /// 尚未 bootstrap。
    Idle,
    /// 主实例正在运行。
    Running,
    /// 已关闭。
    Stopped,
}

/// 启动时探测到的平台能力摘要（业务逻辑不得直接读环境变量做分支）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilitySnapshot {
    pub capture_available: bool,
    pub global_hotkey_available: bool,
    pub clipboard_image_available: bool,
    pub always_on_top_available: bool,
    pub notes: Vec<String>,
}

/// 进程内应用状态。
#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub phase: AppPhase,
    pub capabilities: CapabilitySnapshot,
    pub activation_count: u64,
    /// 已登记的截图（按插入顺序；贴图通过 `image_id` 引用）。
    pub images: Vec<CaptureImage>,
    /// 当前打开的贴图（顺序为创建顺序）。
    pub pins: Vec<Pin>,
    /// 最近一次成功捕获的图像 ID。
    pub last_capture_id: Option<ImageId>,
    /// 最近一次创建的贴图 ID。
    pub last_pin_id: Option<PinId>,
    /// 贴图数量上限（0 表示不限制）。
    pub max_pins: usize,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            phase: AppPhase::Idle,
            capabilities: CapabilitySnapshot::default(),
            activation_count: 0,
            images: Vec::new(),
            pins: Vec::new(),
            last_capture_id: None,
            last_pin_id: None,
            max_pins: 32,
        }
    }

    pub fn pin_count(&self) -> usize {
        self.pins.len()
    }

    pub fn image(&self, id: ImageId) -> Option<&CaptureImage> {
        self.images.iter().find(|img| img.id == id)
    }

    pub fn pin(&self, id: PinId) -> Option<&Pin> {
        self.pins.iter().find(|p| p.id == id)
    }

    pub fn pin_mut(&mut self, id: PinId) -> Option<&mut Pin> {
        self.pins.iter_mut().find(|p| p.id == id)
    }

    /// 登记截图；若 ID 已存在则跳过；更新 last_capture_id。
    pub fn retain_image(&mut self, image: CaptureImage) {
        let id = image.id;
        if self.image(id).is_none() {
            self.images.push(image);
        }
        self.last_capture_id = Some(id);
    }

    /// 从截图创建贴图并加入状态；同时保留图像。
    pub fn create_pin(
        &mut self,
        image: CaptureImage,
        position: PixelPoint,
    ) -> Result<PinId, PinoraError> {
        if self.max_pins > 0 && self.pins.len() >= self.max_pins {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                format!("pin limit reached ({})", self.max_pins),
            ));
        }
        let pin = Pin::from_capture(&image, position);
        let id = pin.id;
        self.retain_image(image);
        self.pins.push(pin);
        self.last_pin_id = Some(id);
        Ok(id)
    }

    /// 从已登记图像创建贴图。
    pub fn create_pin_from_image(
        &mut self,
        image_id: ImageId,
        position: PixelPoint,
    ) -> Result<PinId, PinoraError> {
        if self.max_pins > 0 && self.pins.len() >= self.max_pins {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                format!("pin limit reached ({})", self.max_pins),
            ));
        }
        let image = self.image(image_id).cloned().ok_or_else(|| {
            PinoraError::new(ErrorCode::NotFound, format!("image not found: {image_id}"))
        })?;
        let pin = Pin::from_capture(&image, position);
        let id = pin.id;
        self.pins.push(pin);
        self.last_pin_id = Some(id);
        Ok(id)
    }

    pub fn close_pin(&mut self, id: PinId) -> Result<(), PinoraError> {
        let before = self.pins.len();
        let image_id = self.pin(id).map(|p| p.image_id);
        self.pins.retain(|p| p.id != id);
        if self.pins.len() == before {
            return Err(PinoraError::new(
                ErrorCode::NotFound,
                format!("pin not found: {id}"),
            ));
        }
        if let Some(image_id) = image_id {
            self.drop_image_if_unused(image_id);
        }
        Ok(())
    }

    /// 原子替换既有贴图的图片引用，保留其 `PinId`、交互状态和窗口变换。
    pub fn replace_pin_image(&mut self, id: PinId, image: CaptureImage) -> Result<(), PinoraError> {
        let old_image_id = self
            .pin(id)
            .map(|pin| pin.image_id)
            .ok_or_else(|| PinoraError::new(ErrorCode::NotFound, format!("pin not found: {id}")))?;
        let new_image_id = image.id;
        self.retain_image(image);
        let pin = self
            .pin_mut(id)
            .expect("pin was validated before retaining the replacement image");
        pin.image_id = new_image_id;
        if old_image_id != new_image_id {
            self.drop_image_if_unused(old_image_id);
        }
        Ok(())
    }

    pub fn set_pin_locked(&mut self, id: PinId, locked: bool) -> Result<(), PinoraError> {
        let pin = self
            .pin_mut(id)
            .ok_or_else(|| PinoraError::new(ErrorCode::NotFound, format!("pin not found: {id}")))?;
        pin.set_locked(locked);
        Ok(())
    }

    pub fn set_pin_always_on_top(
        &mut self,
        id: PinId,
        always_on_top: bool,
    ) -> Result<(), PinoraError> {
        let pin = self
            .pin_mut(id)
            .ok_or_else(|| PinoraError::new(ErrorCode::NotFound, format!("pin not found: {id}")))?;
        pin.set_always_on_top(always_on_top);
        Ok(())
    }

    pub fn set_pin_transform(
        &mut self,
        id: PinId,
        transform: PinTransform,
    ) -> Result<(), PinoraError> {
        let pin = self
            .pin_mut(id)
            .ok_or_else(|| PinoraError::new(ErrorCode::NotFound, format!("pin not found: {id}")))?;
        if pin.locked {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                format!("pin is locked: {id}"),
            ));
        }
        pin.transform = transform.clamped();
        Ok(())
    }

    fn drop_image_if_unused(&mut self, image_id: ImageId) {
        let still_used = self.pins.iter().any(|p| p.image_id == image_id);
        if !still_used {
            self.images.retain(|img| img.id != image_id);
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{PixelRect, PixelSize};
    use crate::ids::ImageId;
    use crate::image::{CaptureMetadata, DisplayId, RgbaBuffer};

    fn sample_image() -> CaptureImage {
        let pixels = RgbaBuffer::solid(PixelSize::new(8, 8), [255, 255, 255, 255]);
        CaptureImage::new(
            ImageId::new(),
            pixels,
            PixelRect::new(0, 0, 8, 8),
            CaptureMetadata::new(DisplayId::new("d0"), 1.0, 0),
        )
        .unwrap()
    }

    #[test]
    fn default_state_is_idle() {
        let state = AppState::new();
        assert_eq!(state.phase, AppPhase::Idle);
        assert_eq!(state.activation_count, 0);
        assert!(state.pins.is_empty());
        assert!(state.images.is_empty());
    }

    #[test]
    fn create_and_close_pin() {
        let mut state = AppState::new();
        let image = sample_image();
        let image_id = image.id;
        let id = state
            .create_pin(image, PixelPoint::new(1, 2))
            .expect("create");
        assert_eq!(state.pin_count(), 1);
        assert!(state.pin(id).is_some());
        assert!(state.image(image_id).is_some());
        state.close_pin(id).expect("close");
        assert_eq!(state.pin_count(), 0);
        assert!(state.image(image_id).is_none());
    }

    #[test]
    fn close_missing_pin_errors() {
        let mut state = AppState::new();
        let err = state.close_pin(PinId::from_raw(999)).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[test]
    fn locked_pin_rejects_transform() {
        let mut state = AppState::new();
        let image = sample_image();
        let id = state.create_pin(image, PixelPoint::new(0, 0)).unwrap();
        state.pin_mut(id).unwrap().set_locked(true);
        let err = state
            .set_pin_transform(id, PinTransform::default_at(PixelPoint::new(9, 9)))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::CommandRejected);
    }

    #[test]
    fn pin_limit_enforced() {
        let mut state = AppState::new();
        state.max_pins = 1;
        state
            .create_pin(sample_image(), PixelPoint::new(0, 0))
            .unwrap();
        let err = state
            .create_pin(sample_image(), PixelPoint::new(1, 1))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::CommandRejected);
    }

    #[test]
    fn replacing_a_pin_image_preserves_the_pin_and_releases_the_old_image() {
        let mut state = AppState::new();
        let old = sample_image();
        let old_id = old.id;
        let pin_id = state.create_pin(old, PixelPoint::new(9, 10)).unwrap();
        state.set_pin_locked(pin_id, true).unwrap();
        state.set_pin_always_on_top(pin_id, false).unwrap();
        let replacement = sample_image();
        let replacement_id = replacement.id;

        state.replace_pin_image(pin_id, replacement).unwrap();

        let pin = state.pin(pin_id).unwrap();
        assert_eq!(pin.id, pin_id);
        assert_eq!(pin.image_id, replacement_id);
        assert_eq!(pin.transform.position, PixelPoint::new(9, 10));
        assert!(pin.locked);
        assert!(!pin.always_on_top);
        assert!(state.image(replacement_id).is_some());
        assert!(state.image(old_id).is_none());
    }
}
