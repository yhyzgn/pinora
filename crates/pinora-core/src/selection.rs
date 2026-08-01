//! 区域选区纯逻辑（与窗口/平台无关）。

use crate::error::{ErrorCode, PinoraError};
use crate::geometry::{PixelPoint, PixelRect, PixelSize};

/// 默认最小选区边长（物理像素）。
pub const MIN_SELECTION_EDGE: u32 = 2;

/// 选区拖拽会话。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectionSession {
    pub anchor: Option<PixelPoint>,
    pub cursor: Option<PixelPoint>,
    pub min_edge: u32,
    /// 可选边界（如显示器本地 0,0-w,h）；选区会被 clamp。
    pub bounds: Option<PixelRect>,
}

/// 选区结束结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionOutcome {
    Confirmed(PixelRect),
    Cancelled,
}

impl SelectionSession {
    pub fn new() -> Self {
        Self {
            anchor: None,
            cursor: None,
            min_edge: MIN_SELECTION_EDGE,
            bounds: None,
        }
    }

    pub fn with_bounds(mut self, bounds: PixelRect) -> Self {
        self.bounds = Some(bounds);
        self
    }

    pub fn with_min_edge(mut self, min_edge: u32) -> Self {
        self.min_edge = min_edge.max(1);
        self
    }

    pub fn begin_drag(&mut self, point: PixelPoint) {
        let p = self.clamp_point(point);
        self.anchor = Some(p);
        self.cursor = Some(p);
    }

    pub fn update_cursor(&mut self, point: PixelPoint) {
        self.cursor = Some(self.clamp_point(point));
    }

    pub fn clear(&mut self) {
        self.anchor = None;
        self.cursor = None;
    }

    pub fn has_anchor(&self) -> bool {
        self.anchor.is_some()
    }

    /// 当前预览矩形（可能小于最小尺寸）。
    pub fn preview_rect(&self) -> Option<PixelRect> {
        let a = self.anchor?;
        let b = self.cursor?;
        Some(normalize_rect(a, b))
    }

    /// 尝试确认；过小则失败。
    pub fn try_confirm(&self) -> Result<PixelRect, PinoraError> {
        let rect = self
            .preview_rect()
            .ok_or_else(|| PinoraError::new(ErrorCode::CommandRejected, "no selection started"))?;
        validate_min_size(rect, self.min_edge)?;
        if let Some(bounds) = self.bounds {
            rect.clamp_to(bounds).ok_or_else(|| {
                PinoraError::new(ErrorCode::CommandRejected, "selection outside bounds")
            })
        } else {
            Ok(rect)
        }
    }

    /// 方向键微调：有选区时平移整个选区，否则移动光标。
    pub fn nudge(&mut self, dx: i32, dy: i32) {
        if let (Some(a), Some(c)) = (self.anchor, self.cursor) {
            let na = PixelPoint::new(a.x + dx, a.y + dy);
            let nc = PixelPoint::new(c.x + dx, c.y + dy);
            // 若有 bounds，保持矩形在 bounds 内
            if let Some(bounds) = self.bounds {
                let rect = normalize_rect(na, nc);
                if rect.clamp_to(bounds).is_some()
                    && rect.origin.x >= bounds.origin.x
                    && rect.origin.y >= bounds.origin.y
                    && rect.right() <= bounds.right()
                    && rect.bottom() <= bounds.bottom()
                {
                    self.anchor = Some(na);
                    self.cursor = Some(nc);
                }
            } else {
                self.anchor = Some(na);
                self.cursor = Some(nc);
            }
        } else if let Some(c) = self.cursor {
            self.cursor = Some(self.clamp_point(PixelPoint::new(c.x + dx, c.y + dy)));
        }
    }

    fn clamp_point(&self, point: PixelPoint) -> PixelPoint {
        let Some(b) = self.bounds else {
            return point;
        };
        let max_x = b.right().saturating_sub(1);
        let max_y = b.bottom().saturating_sub(1);
        PixelPoint::new(
            point.x.clamp(b.origin.x, max_x),
            point.y.clamp(b.origin.y, max_y),
        )
    }
}

/// 由对角两点生成规范矩形（左上 + 正尺寸）。
pub fn normalize_rect(a: PixelPoint, b: PixelPoint) -> PixelRect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let width = (x1 - x0).max(0) as u32;
    let height = (y1 - y0).max(0) as u32;
    // 包含终点像素：宽高至少覆盖两端
    let width = width.saturating_add(1);
    let height = height.saturating_add(1);
    PixelRect::new(x0, y0, width, height)
}

pub fn validate_min_size(rect: PixelRect, min_edge: u32) -> Result<(), PinoraError> {
    if rect.size.width < min_edge || rect.size.height < min_edge {
        return Err(PinoraError::new(
            ErrorCode::CommandRejected,
            format!(
                "selection too small ({}x{}), need at least {min_edge}x{min_edge}",
                rect.size.width, rect.size.height
            ),
        ));
    }
    Ok(())
}

/// 将选区限制在图像本地坐标系（0,0)-(w,h)）。
pub fn clamp_to_image(rect: PixelRect, image_size: PixelSize) -> Option<PixelRect> {
    let bounds = PixelRect::new(0, 0, image_size.width, image_size.height);
    rect.clamp_to(bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_order_independent() {
        let a = PixelPoint::new(10, 20);
        let b = PixelPoint::new(5, 8);
        let r = normalize_rect(a, b);
        assert_eq!(r, PixelRect::new(5, 8, 6, 13));
    }

    #[test]
    fn try_confirm_rejects_too_small() {
        let mut s = SelectionSession::new().with_min_edge(2);
        s.begin_drag(PixelPoint::new(0, 0));
        s.update_cursor(PixelPoint::new(0, 0)); // 1x1
        assert!(s.try_confirm().is_err());
    }

    #[test]
    fn try_confirm_accepts_min_edge() {
        let mut s = SelectionSession::new().with_min_edge(2);
        s.begin_drag(PixelPoint::new(0, 0));
        s.update_cursor(PixelPoint::new(1, 1)); // 2x2 inclusive
        let r = s.try_confirm().unwrap();
        assert_eq!(r.size, PixelSize::new(2, 2));
    }

    #[test]
    fn nudge_moves_selection() {
        let mut s = SelectionSession::new()
            .with_bounds(PixelRect::new(0, 0, 100, 100))
            .with_min_edge(2);
        s.begin_drag(PixelPoint::new(10, 10));
        s.update_cursor(PixelPoint::new(20, 20));
        s.nudge(5, -3);
        let r = s.preview_rect().unwrap();
        assert_eq!(r.origin, PixelPoint::new(15, 7));
    }

    #[test]
    fn clamp_to_image_works() {
        let r = PixelRect::new(-5, 10, 50, 20);
        let c = clamp_to_image(r, PixelSize::new(100, 100)).unwrap();
        assert_eq!(c, PixelRect::new(0, 10, 45, 20));
    }
}
