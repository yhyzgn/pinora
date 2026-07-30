//! 像素几何类型（图像坐标系，原点在左上，单位为物理像素）。

/// 二维整数点。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PixelPoint {
    pub x: i32,
    pub y: i32,
}

impl PixelPoint {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// 宽高尺寸（非负）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

impl PixelSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn area(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

/// 轴对齐矩形（左上原点 + 尺寸）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PixelRect {
    pub origin: PixelPoint,
    pub size: PixelSize,
}

impl PixelRect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            origin: PixelPoint::new(x, y),
            size: PixelSize::new(width, height),
        }
    }

    pub fn right(self) -> i32 {
        self.origin.x.saturating_add(self.size.width as i32)
    }

    pub fn bottom(self) -> i32 {
        self.origin.y.saturating_add(self.size.height as i32)
    }

    pub fn contains_point(self, point: PixelPoint) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && point.x < self.right()
            && point.y < self.bottom()
    }

    /// 将矩形裁剪到边界内；无交集时返回 `None`。
    pub fn clamp_to(self, bounds: PixelRect) -> Option<PixelRect> {
        let x0 = self.origin.x.max(bounds.origin.x);
        let y0 = self.origin.y.max(bounds.origin.y);
        let x1 = self.right().min(bounds.right());
        let y1 = self.bottom().min(bounds.bottom());
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(PixelRect::new(
            x0,
            y0,
            (x1 - x0) as u32,
            (y1 - y0) as u32,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_point_respects_half_open_edges() {
        let rect = PixelRect::new(10, 20, 100, 50);
        assert!(rect.contains_point(PixelPoint::new(10, 20)));
        assert!(rect.contains_point(PixelPoint::new(109, 69)));
        assert!(!rect.contains_point(PixelPoint::new(110, 20)));
        assert!(!rect.contains_point(PixelPoint::new(10, 70)));
    }

    #[test]
    fn clamp_to_returns_intersection() {
        let outer = PixelRect::new(0, 0, 1920, 1080);
        let selection = PixelRect::new(-50, 100, 200, 100);
        let clamped = selection.clamp_to(outer).expect("intersection");
        assert_eq!(clamped, PixelRect::new(0, 100, 150, 100));
    }

    #[test]
    fn clamp_to_none_when_disjoint() {
        let a = PixelRect::new(0, 0, 10, 10);
        let b = PixelRect::new(20, 20, 5, 5);
        assert!(a.clamp_to(b).is_none());
    }
}
