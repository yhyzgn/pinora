use crate::error::{ErrorCode, PinoraError};
use crate::geometry::PixelPoint;
use crate::ids::PinId;
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
    /// 当前打开的贴图（顺序为创建顺序）。
    pub pins: Vec<Pin>,
    /// 贴图数量上限（0 表示不限制）。
    pub max_pins: usize,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            phase: AppPhase::Idle,
            capabilities: CapabilitySnapshot::default(),
            activation_count: 0,
            pins: Vec::new(),
            max_pins: 32,
        }
    }

    pub fn pin_count(&self) -> usize {
        self.pins.len()
    }

    pub fn pin(&self, id: PinId) -> Option<&Pin> {
        self.pins.iter().find(|p| p.id == id)
    }

    pub fn pin_mut(&mut self, id: PinId) -> Option<&mut Pin> {
        self.pins.iter_mut().find(|p| p.id == id)
    }

    /// 从截图创建贴图并加入状态。
    pub fn create_pin(
        &mut self,
        image: &CaptureImage,
        position: PixelPoint,
    ) -> Result<PinId, PinoraError> {
        if self.max_pins > 0 && self.pins.len() >= self.max_pins {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                format!("pin limit reached ({})", self.max_pins),
            ));
        }
        let pin = Pin::from_capture(image, position);
        let id = pin.id;
        self.pins.push(pin);
        Ok(id)
    }

    pub fn close_pin(&mut self, id: PinId) -> Result<(), PinoraError> {
        let before = self.pins.len();
        self.pins.retain(|p| p.id != id);
        if self.pins.len() == before {
            return Err(PinoraError::new(
                ErrorCode::NotFound,
                format!("pin not found: {id}"),
            ));
        }
        Ok(())
    }

    pub fn set_pin_transform(
        &mut self,
        id: PinId,
        transform: PinTransform,
    ) -> Result<(), PinoraError> {
        let pin = self.pin_mut(id).ok_or_else(|| {
            PinoraError::new(ErrorCode::NotFound, format!("pin not found: {id}"))
        })?;
        if pin.locked {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                format!("pin is locked: {id}"),
            ));
        }
        pin.transform = transform.clamped();
        Ok(())
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
    }

    #[test]
    fn create_and_close_pin() {
        let mut state = AppState::new();
        let image = sample_image();
        let id = state
            .create_pin(&image, PixelPoint::new(1, 2))
            .expect("create");
        assert_eq!(state.pin_count(), 1);
        assert!(state.pin(id).is_some());
        state.close_pin(id).expect("close");
        assert_eq!(state.pin_count(), 0);
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
        let id = state.create_pin(&image, PixelPoint::new(0, 0)).unwrap();
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
        let image = sample_image();
        state.create_pin(&image, PixelPoint::new(0, 0)).unwrap();
        let err = state
            .create_pin(&image, PixelPoint::new(1, 1))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::CommandRejected);
    }
}
