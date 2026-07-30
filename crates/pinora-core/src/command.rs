use std::path::PathBuf;

use crate::action::ActionId;
use crate::capture::CaptureRequest;
use crate::geometry::PixelPoint;
use crate::ids::{CorrelationId, ImageId, PinId};
use crate::image::CaptureImage;
use crate::pin::PinTransform;

/// 用户或系统意图。命令可以失败；成功后应产生对应领域事件。
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Bootstrap { correlation_id: CorrelationId },
    Activate { correlation_id: CorrelationId },
    Shutdown { correlation_id: CorrelationId },
    Capture {
        correlation_id: CorrelationId,
        request: CaptureRequest,
    },
    /// 捕获并立即创建贴图。
    CaptureAndPin {
        correlation_id: CorrelationId,
        request: CaptureRequest,
        position: PixelPoint,
    },
    CreatePin {
        correlation_id: CorrelationId,
        image: CaptureImage,
        position: PixelPoint,
    },
    CreatePinFromImage {
        correlation_id: CorrelationId,
        image_id: ImageId,
        position: PixelPoint,
    },
    ClosePin {
        correlation_id: CorrelationId,
        pin_id: PinId,
    },
    SetPinTransform {
        correlation_id: CorrelationId,
        pin_id: PinId,
        transform: PinTransform,
    },
    /// 将已登记图像保存为 PNG。
    SavePng {
        correlation_id: CorrelationId,
        image_id: ImageId,
        path: PathBuf,
    },
    /// 将已登记图像复制到 ImageSink 剪贴板。
    CopyImage {
        correlation_id: CorrelationId,
        image_id: ImageId,
    },
    /// 执行高层动作（由热键/托盘映射）。
    InvokeAction {
        correlation_id: CorrelationId,
        action: ActionId,
    },
}

impl Command {
    pub fn correlation_id(&self) -> CorrelationId {
        match self {
            Self::Bootstrap { correlation_id }
            | Self::Activate { correlation_id }
            | Self::Shutdown { correlation_id }
            | Self::Capture { correlation_id, .. }
            | Self::CaptureAndPin { correlation_id, .. }
            | Self::CreatePin { correlation_id, .. }
            | Self::CreatePinFromImage { correlation_id, .. }
            | Self::ClosePin { correlation_id, .. }
            | Self::SetPinTransform { correlation_id, .. }
            | Self::SavePng { correlation_id, .. }
            | Self::CopyImage { correlation_id, .. }
            | Self::InvokeAction { correlation_id, .. } => *correlation_id,
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

    pub fn capture(request: CaptureRequest) -> Self {
        Self::Capture {
            correlation_id: CorrelationId::new(),
            request,
        }
    }

    pub fn capture_and_pin(request: CaptureRequest, position: PixelPoint) -> Self {
        Self::CaptureAndPin {
            correlation_id: CorrelationId::new(),
            request,
            position,
        }
    }

    pub fn create_pin(image: CaptureImage, position: PixelPoint) -> Self {
        Self::CreatePin {
            correlation_id: CorrelationId::new(),
            image,
            position,
        }
    }

    pub fn create_pin_from_image(image_id: ImageId, position: PixelPoint) -> Self {
        Self::CreatePinFromImage {
            correlation_id: CorrelationId::new(),
            image_id,
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

    pub fn save_png(image_id: ImageId, path: impl Into<PathBuf>) -> Self {
        Self::SavePng {
            correlation_id: CorrelationId::new(),
            image_id,
            path: path.into(),
        }
    }

    pub fn copy_image(image_id: ImageId) -> Self {
        Self::CopyImage {
            correlation_id: CorrelationId::new(),
            image_id,
        }
    }

    pub fn invoke_action(action: ActionId) -> Self {
        Self::InvokeAction {
            correlation_id: CorrelationId::new(),
            action,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{PixelRect, PixelSize};
    use crate::image::{CaptureMetadata, DisplayId, RgbaBuffer};

    #[test]
    fn commands_carry_correlation_ids() {
        let cmd = Command::bootstrap();
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
