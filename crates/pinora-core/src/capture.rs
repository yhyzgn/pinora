//! 截图能力抽象与请求类型（无平台 SDK 依赖）。

use std::fmt;

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

/// 平台后端在当前会话中分配的窗口身份。
///
/// 仅用于同一后端内重新验证窗口快照，绝不应写入日志、历史、菜单文本或持久化数据。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaptureWindowId(u64);

impl CaptureWindowId {
    pub fn from_raw(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Debug for CaptureWindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CaptureWindowId(<redacted>)")
    }
}

/// 可被当前捕获后端验证的窗口快照。
///
/// 文本字段只能用于本地菜单显示，调用方必须清洗并截断，且不得把它们写入日志或历史。
#[derive(Clone, PartialEq)]
pub struct CaptureWindowInfo {
    pub id: CaptureWindowId,
    pub app_name: String,
    pub title: String,
    pub bounds: PixelRect,
    pub display: DisplayId,
    pub scale: f64,
    pub is_minimized: bool,
}

impl CaptureWindowInfo {
    /// 目标必须在点击时仍是同一窗口、同一显示器上下文和同一几何；最小化窗口不允许
    /// 作为捕获对象。标题或应用名变化不会让同一窗口错误降级为另一个目标。
    pub fn matches_capture_snapshot(&self, current: &Self) -> bool {
        self.id == current.id
            && self.bounds == current.bounds
            && self.display == current.display
            && self.scale.to_bits() == current.scale.to_bits()
            && !current.is_minimized
    }
}

impl fmt::Debug for CaptureWindowInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureWindowInfo")
            .field("id", &self.id)
            .field("bounds", &self.bounds)
            .field("display", &self.display)
            .field("scale", &self.scale)
            .field("is_minimized", &self.is_minimized)
            .finish()
    }
}

/// 捕获请求。
#[derive(Debug, Clone, PartialEq)]
pub enum CaptureRequest {
    /// 指定显示器上的矩形区域（图像坐标，需落在显示器 bounds 内或可裁剪）。
    Region { display: DisplayId, rect: PixelRect },
    /// 整屏捕获。
    FullDisplay { display: DisplayId },
    /// 按已验证的窗口快照捕获；后端必须在取得像素前重新验证该快照。
    Window { target: CaptureWindowInfo },
}

/// 截图提供者。业务层只依赖此 trait；测试注入 fake。
pub trait CaptureProvider {
    fn displays(&self) -> Result<Vec<DisplayInfo>, PinoraError>;
    fn windows(&self) -> Result<Vec<CaptureWindowInfo>, PinoraError>;
    fn capture(&self, request: CaptureRequest) -> Result<CaptureImage, PinoraError>;
}

impl CaptureRequest {
    pub fn display_id(&self) -> &DisplayId {
        match self {
            Self::Region { display, .. } | Self::FullDisplay { display } => display,
            Self::Window { target } => &target.display,
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
        CaptureRequest::Region { rect, .. } => rect.clamp_to(info.bounds).ok_or_else(|| {
            PinoraError::new(
                ErrorCode::CommandRejected,
                "capture region does not intersect display",
            )
        })?,
        CaptureRequest::Window { .. } => {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                "window capture must be resolved by its platform provider",
            ));
        }
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
        let (info, rect) = resolve_capture_rect(std::slice::from_ref(&d), &req).unwrap();
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

    #[test]
    fn window_capture_requires_a_platform_specific_resolution_path() {
        let display = sample_display();
        let target = CaptureWindowInfo {
            id: CaptureWindowId::from_raw(7),
            app_name: "Example".into(),
            title: "Sensitive title".into(),
            bounds: PixelRect::new(10, 20, 100, 50),
            display: display.id.clone(),
            scale: 1.0,
            is_minimized: false,
        };

        let error = resolve_capture_rect(
            &[display],
            &CaptureRequest::Window {
                target: target.clone(),
            },
        )
        .expect_err("window capture must not become a display capture");
        assert_eq!(error.code, ErrorCode::CommandRejected);
        let debug = format!("{target:?}");
        assert!(!debug.contains("Sensitive title"));
        assert!(!debug.contains("Example"));
    }

    #[test]
    fn window_target_revalidation_requires_geometry_and_display_to_match() {
        let base = CaptureWindowInfo {
            id: CaptureWindowId::from_raw(7),
            app_name: "Example".into(),
            title: "First title".into(),
            bounds: PixelRect::new(10, 20, 100, 50),
            display: DisplayId::new("d0"),
            scale: 1.0,
            is_minimized: false,
        };
        let mut title_changed = base.clone();
        title_changed.title = "Second title".into();
        assert!(base.matches_capture_snapshot(&title_changed));

        let mut moved = base.clone();
        moved.bounds = PixelRect::new(11, 20, 100, 50);
        assert!(!base.matches_capture_snapshot(&moved));

        let mut minimized = base.clone();
        minimized.is_minimized = true;
        assert!(!base.matches_capture_snapshot(&minimized));
    }
}
