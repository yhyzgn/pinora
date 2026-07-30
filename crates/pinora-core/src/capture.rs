//! 截图能力抽象与请求类型（无平台 SDK 依赖）。

use crate::error::{ErrorCode, PinoraError};
use crate::geometry::PixelRect;
use crate::image::{CaptureImage, DisplayId};

/// 显示器信息（平台无关）。
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    pub id: DisplayId,
    pub name: String,
    pub bounds: PixelRect,
    pub scale: f64,
}

/// 捕获请求。
#[derive(Debug, Clone, PartialEq)]
pub enum CaptureRequest {
    /// 指定显示器上的矩形区域（图像坐标，需落在显示器 bounds 内或可裁剪）。
    Region {
        display: DisplayId,
        rect: PixelRect,
    },
    /// 整屏捕获。
    FullDisplay {
        display: DisplayId,
    },
}

/// 截图提供者。业务层只依赖此 trait；测试注入 fake。
pub trait CaptureProvider {
    fn displays(&self) -> Result<Vec<DisplayInfo>, PinoraError>;
    fn capture(&self, request: CaptureRequest) -> Result<CaptureImage, PinoraError>;
}

impl CaptureRequest {
    pub fn display_id(&self) -> &DisplayId {
        match self {
            Self::Region { display, .. } | Self::FullDisplay { display } => display,
        }
    }
}

/// 解析请求为最终捕获矩形（相对显示器局部坐标转全局时由实现处理）。
pub fn resolve_capture_rect(
    displays: &[DisplayInfo],
    request: &CaptureRequest,
) -> Result<(DisplayInfo, PixelRect), PinoraError> {
    let display_id = request.display_id();
    let info = displays
        .iter()
        .find(|d| &d.id == display_id)
        .cloned()
        .ok_or_else(|| {
            PinoraError::new(
                ErrorCode::NotFound,
                format!("display not found: {}", display_id.0),
            )
        })?;

    let rect = match request {
        CaptureRequest::FullDisplay { .. } => info.bounds,
        CaptureRequest::Region { rect, .. } => rect
            .clamp_to(info.bounds)
            .ok_or_else(|| {
                PinoraError::new(
                    ErrorCode::CommandRejected,
                    "capture region does not intersect display",
                )
            })?,
    };

    if rect.size.is_empty() {
        return Err(PinoraError::new(
            ErrorCode::CommandRejected,
            "capture region is empty",
        ));
    }

    Ok((info, rect))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::PixelRect;
    use crate::image::DisplayId;

    fn sample_display() -> DisplayInfo {
        DisplayInfo {
            id: DisplayId::new("d0"),
            name: "Primary".into(),
            bounds: PixelRect::new(0, 0, 1920, 1080),
            scale: 1.0,
        }
    }

    #[test]
    fn resolve_full_display() {
        let d = sample_display();
        let req = CaptureRequest::FullDisplay {
            display: DisplayId::new("d0"),
        };
        let (info, rect) = resolve_capture_rect(&[d.clone()], &req).unwrap();
        assert_eq!(info.id, d.id);
        assert_eq!(rect, d.bounds);
    }

    #[test]
    fn resolve_region_clamped() {
        let d = sample_display();
        let req = CaptureRequest::Region {
            display: DisplayId::new("d0"),
            rect: PixelRect::new(-10, 100, 50, 40),
        };
        let (_, rect) = resolve_capture_rect(&[d], &req).unwrap();
        assert_eq!(rect, PixelRect::new(0, 100, 40, 40));
    }
}
