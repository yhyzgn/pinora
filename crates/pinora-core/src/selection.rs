//! 区域选区纯逻辑（与窗口/平台无关）。

use crate::error::{ErrorCode, PinoraError};
use crate::geometry::{PixelPoint, PixelRect, PixelSize};

/// 默认最小选区边长（物理像素）。
pub const MIN_SELECTION_EDGE: u32 = 2;

/// 已确认选区的可调整边或角。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionHandle {
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

impl SelectionHandle {
    pub const ALL: [Self; 8] = [
        Self::NorthWest,
        Self::NorthEast,
        Self::SouthEast,
        Self::SouthWest,
        Self::North,
        Self::East,
        Self::South,
        Self::West,
    ];

    /// 返回此热区在选区边框上的物理像素中心。
    pub fn center(self, rect: PixelRect) -> PixelPoint {
        let left = rect.origin.x;
        let top = rect.origin.y;
        let right = rect.right().saturating_sub(1);
        let bottom = rect.bottom().saturating_sub(1);
        match self {
            Self::NorthWest => PixelPoint::new(left, top),
            Self::North => {
                PixelPoint::new(left.saturating_add(right.saturating_sub(left) / 2), top)
            }
            Self::NorthEast => PixelPoint::new(right, top),
            Self::East => {
                PixelPoint::new(right, top.saturating_add(bottom.saturating_sub(top) / 2))
            }
            Self::SouthEast => PixelPoint::new(right, bottom),
            Self::South => {
                PixelPoint::new(left.saturating_add(right.saturating_sub(left) / 2), bottom)
            }
            Self::SouthWest => PixelPoint::new(left, bottom),
            Self::West => PixelPoint::new(left, top.saturating_add(bottom.saturating_sub(top) / 2)),
        }
    }

    const fn moves_west(self) -> bool {
        matches!(self, Self::NorthWest | Self::SouthWest | Self::West)
    }

    const fn moves_east(self) -> bool {
        matches!(self, Self::NorthEast | Self::SouthEast | Self::East)
    }

    const fn moves_north(self) -> bool {
        matches!(self, Self::NorthWest | Self::NorthEast | Self::North)
    }

    const fn moves_south(self) -> bool {
        matches!(self, Self::SouthWest | Self::SouthEast | Self::South)
    }
}

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

    /// 选中整个受限区域，返回经过最小尺寸校验的矩形。
    pub fn select_all(&mut self) -> Result<PixelRect, PinoraError> {
        let bounds = self.bounds.ok_or_else(|| {
            PinoraError::new(ErrorCode::CommandRejected, "selection bounds are required")
        })?;
        if bounds.size.is_empty() {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                "selection bounds are empty",
            ));
        }
        self.begin_drag(bounds.origin);
        self.update_cursor(PixelPoint::new(
            bounds.right().saturating_sub(1),
            bounds.bottom().saturating_sub(1),
        ));
        self.try_confirm()
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

    /// 调整当前已确认选区的一条边或一个角。
    ///
    /// 调整点始终限制在画布 bounds 内，且边长不会小于 `min_edge`。拖过对边时，
    /// 被拖动的一侧停在最小尺寸处，选区不会翻转。
    pub fn resize_from_handle(&mut self, handle: SelectionHandle, point: PixelPoint) -> bool {
        let Some(rect) = self.preview_rect() else {
            return false;
        };
        if validate_min_size(rect, self.min_edge).is_err() {
            return false;
        }

        let point = self.clamp_point(point);
        let min_span = i32::try_from(self.min_edge.saturating_sub(1)).unwrap_or(i32::MAX);
        let mut left = rect.origin.x;
        let mut top = rect.origin.y;
        let mut right = rect.right().saturating_sub(1);
        let mut bottom = rect.bottom().saturating_sub(1);

        if handle.moves_west() {
            left = point.x.min(right.saturating_sub(min_span));
        }
        if handle.moves_east() {
            right = point.x.max(left.saturating_add(min_span));
        }
        if handle.moves_north() {
            top = point.y.min(bottom.saturating_sub(min_span));
        }
        if handle.moves_south() {
            bottom = point.y.max(top.saturating_add(min_span));
        }

        let anchor = PixelPoint::new(left, top);
        let cursor = PixelPoint::new(right, bottom);
        let changed = self.anchor != Some(anchor) || self.cursor != Some(cursor);
        self.anchor = Some(anchor);
        self.cursor = Some(cursor);
        changed
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
    fn every_handle_resizes_its_expected_edges() {
        let cases = [
            (
                SelectionHandle::NorthWest,
                PixelPoint::new(4, 5),
                PixelRect::new(4, 5, 27, 36),
            ),
            (
                SelectionHandle::North,
                PixelPoint::new(99, 8),
                PixelRect::new(10, 8, 21, 33),
            ),
            (
                SelectionHandle::NorthEast,
                PixelPoint::new(35, 7),
                PixelRect::new(10, 7, 26, 34),
            ),
            (
                SelectionHandle::East,
                PixelPoint::new(36, 99),
                PixelRect::new(10, 20, 27, 21),
            ),
            (
                SelectionHandle::SouthEast,
                PixelPoint::new(37, 48),
                PixelRect::new(10, 20, 28, 29),
            ),
            (
                SelectionHandle::South,
                PixelPoint::new(9, 49),
                PixelRect::new(10, 20, 21, 30),
            ),
            (
                SelectionHandle::SouthWest,
                PixelPoint::new(3, 47),
                PixelRect::new(3, 20, 28, 28),
            ),
            (
                SelectionHandle::West,
                PixelPoint::new(2, 99),
                PixelRect::new(2, 20, 29, 21),
            ),
        ];

        for (handle, point, expected) in cases {
            let mut session = SelectionSession::new()
                .with_bounds(PixelRect::new(0, 0, 60, 60))
                .with_min_edge(2);
            session.begin_drag(PixelPoint::new(10, 20));
            session.update_cursor(PixelPoint::new(30, 40));

            assert!(session.resize_from_handle(handle, point), "{handle:?}");
            assert_eq!(session.try_confirm(), Ok(expected), "{handle:?}");
        }
    }

    #[test]
    fn resize_clamps_to_bounds_and_minimum_size_without_flipping() {
        let mut session = SelectionSession::new()
            .with_bounds(PixelRect::new(0, 0, 60, 60))
            .with_min_edge(5);
        session.begin_drag(PixelPoint::new(10, 10));
        session.update_cursor(PixelPoint::new(20, 20));

        assert!(session.resize_from_handle(SelectionHandle::NorthWest, PixelPoint::new(99, 99)));
        assert_eq!(session.try_confirm(), Ok(PixelRect::new(16, 16, 5, 5)));
        assert!(!session.resize_from_handle(SelectionHandle::NorthWest, PixelPoint::new(99, 99)));

        assert!(!session.resize_from_handle(SelectionHandle::SouthEast, PixelPoint::new(-8, -8)));
        assert_eq!(session.try_confirm(), Ok(PixelRect::new(16, 16, 5, 5)));

        let mut bounded = SelectionSession::new()
            .with_bounds(PixelRect::new(0, 0, 60, 60))
            .with_min_edge(2);
        bounded.begin_drag(PixelPoint::new(10, 10));
        bounded.update_cursor(PixelPoint::new(20, 20));
        assert!(bounded.resize_from_handle(SelectionHandle::East, PixelPoint::new(99, 10)));
        assert_eq!(bounded.try_confirm(), Ok(PixelRect::new(10, 10, 50, 11)));
    }

    #[test]
    fn handle_centers_are_on_the_selection_border() {
        let rect = PixelRect::new(10, 20, 21, 11);
        assert_eq!(
            SelectionHandle::NorthWest.center(rect),
            PixelPoint::new(10, 20)
        );
        assert_eq!(SelectionHandle::North.center(rect), PixelPoint::new(20, 20));
        assert_eq!(
            SelectionHandle::NorthEast.center(rect),
            PixelPoint::new(30, 20)
        );
        assert_eq!(SelectionHandle::East.center(rect), PixelPoint::new(30, 25));
        assert_eq!(
            SelectionHandle::SouthEast.center(rect),
            PixelPoint::new(30, 30)
        );
        assert_eq!(SelectionHandle::South.center(rect), PixelPoint::new(20, 30));
        assert_eq!(
            SelectionHandle::SouthWest.center(rect),
            PixelPoint::new(10, 30)
        );
        assert_eq!(SelectionHandle::West.center(rect), PixelPoint::new(10, 25));
    }

    #[test]
    fn select_all_covers_the_full_bounded_image() {
        let mut s = SelectionSession::new()
            .with_bounds(PixelRect::new(-20, 15, 1920, 1080))
            .with_min_edge(2);

        let rect = s.select_all().unwrap();

        assert_eq!(rect, PixelRect::new(-20, 15, 1920, 1080));
    }

    #[test]
    fn clamp_to_image_works() {
        let r = PixelRect::new(-5, 10, 50, 20);
        let c = clamp_to_image(r, PixelSize::new(100, 100)).unwrap();
        assert_eq!(c, PixelRect::new(0, 10, 45, 20));
    }
}
