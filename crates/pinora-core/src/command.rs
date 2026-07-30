use crate::geometry::PixelPoint;
use crate::ids::{CorrelationId, PinId};
use crate::image::CaptureImage;
use crate::pin::PinTransform;

/// 用户或系统意图。命令可以失败；成功后应产生对应领域事件。
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// 启动应用运行时（首实例）。
    Bootstrap { correlation_id: CorrelationId },
    /// 激活已有实例（二次启动转发）。
    Activate { correlation_id: CorrelationId },
    /// 请求优雅退出。
    Shutdown { correlation_id: CorrelationId },
    /// 从截图创建贴图。
    CreatePin {
        correlation_id: CorrelationId,
        image: CaptureImage,
        position: PixelPoint,
    },
    /// 关闭贴图。
    ClosePin {
        correlation_id: CorrelationId,
        pin_id: PinId,
    },
    /// 更新贴图变换。
    SetPinTransform {
        correlation_id: CorrelationId,
        pin_id: PinId,
        transform: PinTransform,
    },
}

impl Command {
    pub fn correlation_id(&self) -> CorrelationId {
        match self {
            Self::Bootstrap { correlation_id }
            | Self::Activate { correlation_id }
            | Self::Shutdown { correlation_id }
            | Self::CreatePin { correlation_id, .. }
            | Self::ClosePin { correlation_id, .. }
            | Self::SetPinTransform { correlation_id, .. } => *correlation_id,
        }
    }

    pub fn bootstrap() -> Self {
        Self::Bootstrap {
            correlation_id: CorrelationId::new(),
        }
    }

    pub fn activate() -> Self {
        Self::Activate {
            correlation_id: CorrelationId::new(),
        }
    }

    pub fn shutdown() -> Self {
        Self::Shutdown {
            correlation_id: CorrelationId::new(),
        }
    }

    pub fn create_pin(image: CaptureImage, position: PixelPoint) -> Self {
        Self::CreatePin {
            correlation_id: CorrelationId::new(),
            image,
            position,
        }
    }

    pub fn close_pin(pin_id: PinId) -> Self {
        Self::ClosePin {
            correlation_id: CorrelationId::new(),
            pin_id,
        }
    }

    pub fn set_pin_transform(pin_id: PinId, transform: PinTransform) -> Self {
        Self::SetPinTransform {
            correlation_id: CorrelationId::new(),
            pin_id,
            transform,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{PixelRect, PixelSize};
    use crate::ids::ImageId;
    use crate::image::{CaptureMetadata, DisplayId, RgbaBuffer};

    #[test]
    fn commands_carry_correlation_ids() {
        let cmd = Command::bootstrap();
        assert!(matches!(cmd, Command::Bootstrap { .. }));
        assert!(cmd.correlation_id().raw() > 0);
    }

    #[test]
    fn create_pin_command_holds_image_size() {
        let pixels = RgbaBuffer::solid(PixelSize::new(4, 4), [1, 2, 3, 4]);
        let image = CaptureImage::new(
            ImageId::new(),
            pixels,
            PixelRect::new(0, 0, 4, 4),
            CaptureMetadata::new(DisplayId::new("d0"), 1.0, 0),
        )
        .unwrap();
        let cmd = Command::create_pin(image, PixelPoint::new(10, 20));
        match cmd {
            Command::CreatePin {
                image, position, ..
            } => {
                assert_eq!(image.size(), PixelSize::new(4, 4));
                assert_eq!(position, PixelPoint::new(10, 20));
            }
            _ => panic!("expected CreatePin"),
        }
    }
}
