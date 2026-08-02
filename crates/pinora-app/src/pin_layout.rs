//! 贴图窗口的纯尺寸计算。
//!
//! 此模块不持有窗口、事件循环或平台资源，因此可由受 tray 监督的桌面壳安全复用。

use pinora_core::{PixelPoint, PixelSize};

/// 贴图缩放的领域兼容范围。与 `PinTransform::clamped` 保持一致。
pub const PIN_MIN_SCALE: f64 = 0.05;
pub const PIN_MAX_SCALE: f64 = 8.0;

/// 无边框贴图用于命中尺寸操作的客户区边距（物理像素）。
pub const PIN_RESIZE_GRIP: u32 = 10;

/// 客户区内可发起原生尺寸操作的八个方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinResizeHandle {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl PinResizeHandle {
    const fn uses_width(self) -> bool {
        matches!(
            self,
            Self::East
                | Self::West
                | Self::NorthEast
                | Self::SouthEast
                | Self::SouthWest
                | Self::NorthWest
        )
    }

    const fn uses_height(self) -> bool {
        matches!(
            self,
            Self::North
                | Self::South
                | Self::NorthEast
                | Self::SouthEast
                | Self::SouthWest
                | Self::NorthWest
        )
    }

    const fn moves_left_edge(self) -> bool {
        matches!(self, Self::West | Self::NorthWest | Self::SouthWest)
    }

    const fn moves_top_edge(self) -> bool {
        matches!(self, Self::North | Self::NorthEast | Self::NorthWest)
    }
}

/// 比例约束后的贴图窗口尺寸与领域缩放。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PinResizeTarget {
    pub scale: f64,
    pub size: PixelSize,
}

/// 返回 100% 原图尺寸对应的贴图窗口目标。
pub fn fit_to_image_target(image: PixelSize) -> PinResizeTarget {
    let (width, height) = scaled_window_size(image, 1.0);
    PinResizeTarget {
        scale: 1.0,
        size: PixelSize::new(width, height),
    }
}

/// 根据图像尺寸与缩放计算窗口物理像素大小。
pub fn scaled_window_size(image: PixelSize, scale: f64) -> (u32, u32) {
    let s = if scale.is_finite() && scale > 0.0 {
        scale.clamp(PIN_MIN_SCALE, PIN_MAX_SCALE)
    } else {
        1.0
    };
    let w = ((f64::from(image.width) * s).round() as u32).max(1);
    let h = ((f64::from(image.height) * s).round() as u32).max(1);
    (w, h)
}

/// 命中贴图客户区的边或角。角优先于边，避免在视觉交界处落入错误方向。
pub fn pin_resize_handle_at(
    point: PixelPoint,
    window_width: u32,
    window_height: u32,
) -> Option<PinResizeHandle> {
    if point.x < 0
        || point.y < 0
        || point.x >= window_width as i32
        || point.y >= window_height as i32
    {
        return None;
    }
    let grip = effective_grip(window_width, window_height) as i32;
    let right_edge = window_width as i32 - grip;
    let bottom_edge = window_height as i32 - grip;
    let west = point.x < grip;
    let east = point.x >= right_edge;
    let north = point.y < grip;
    let south = point.y >= bottom_edge;

    match (north, south, west, east) {
        (true, _, true, _) => Some(PinResizeHandle::NorthWest),
        (true, _, _, true) => Some(PinResizeHandle::NorthEast),
        (_, true, true, _) => Some(PinResizeHandle::SouthWest),
        (_, true, _, true) => Some(PinResizeHandle::SouthEast),
        (true, _, _, _) => Some(PinResizeHandle::North),
        (_, true, _, _) => Some(PinResizeHandle::South),
        (_, _, true, _) => Some(PinResizeHandle::West),
        (_, _, _, true) => Some(PinResizeHandle::East),
        _ => None,
    }
}

/// 将平台实际报告的窗口尺寸重新约束为原图比例。
///
/// 单边操作只采用该边对应的轴；角操作选择相对当前比例变化较大的轴，避免平台在
/// 连续 configure 中对另一轴的微小舍入扰动反向拉扯尺寸。
pub fn proportional_resize_target(
    image: PixelSize,
    current_scale: f64,
    observed_size: PixelSize,
    handle: PinResizeHandle,
) -> PinResizeTarget {
    let image_width = image.width.max(1) as f64;
    let image_height = image.height.max(1) as f64;
    let width_scale = observed_size.width.max(1) as f64 / image_width;
    let height_scale = observed_size.height.max(1) as f64 / image_height;
    let current = normalized_scale(current_scale);
    let scale = match (handle.uses_width(), handle.uses_height()) {
        (true, false) => width_scale,
        (false, true) => height_scale,
        (true, true) if (width_scale - current).abs() >= (height_scale - current).abs() => {
            width_scale
        }
        (true, true) => height_scale,
        (false, false) => current,
    }
    .clamp(PIN_MIN_SCALE, PIN_MAX_SCALE);
    let (width, height) = scaled_window_size(image, scale);
    PinResizeTarget {
        scale,
        size: PixelSize::new(width, height),
    }
}

/// 为不支持原生 interactive resize 的平台计算手动拖动目标。
pub fn pin_resize_target_from_drag(
    image: PixelSize,
    current_scale: f64,
    start_size: PixelSize,
    start_cursor: (f64, f64),
    cursor: (f64, f64),
    handle: PinResizeHandle,
) -> PinResizeTarget {
    let delta_x = (cursor.0 - start_cursor.0).round() as i64;
    let delta_y = (cursor.1 - start_cursor.1).round() as i64;
    let width = if handle.moves_left_edge() {
        i64::from(start_size.width) - delta_x
    } else if handle.uses_width() {
        i64::from(start_size.width) + delta_x
    } else {
        i64::from(start_size.width)
    }
    .clamp(1, i64::from(u32::MAX)) as u32;
    let height = if handle.moves_top_edge() {
        i64::from(start_size.height) - delta_y
    } else if handle.uses_height() {
        i64::from(start_size.height) + delta_y
    } else {
        i64::from(start_size.height)
    }
    .clamp(1, i64::from(u32::MAX)) as u32;
    proportional_resize_target(image, current_scale, PixelSize::new(width, height), handle)
}

/// 当左边或上边被拖动时，保持原先对边/对角锚点不动的窗口左上坐标。
pub fn pin_resize_anchor_position(
    handle: PinResizeHandle,
    start_position: PixelPoint,
    start_size: PixelSize,
    target_size: PixelSize,
) -> PixelPoint {
    let x = if handle.moves_left_edge() {
        start_position
            .x
            .saturating_add(start_size.width as i32)
            .saturating_sub(target_size.width as i32)
    } else {
        start_position.x
    };
    let y = if handle.moves_top_edge() {
        start_position
            .y
            .saturating_add(start_size.height as i32)
            .saturating_sub(target_size.height as i32)
    } else {
        start_position.y
    };
    PixelPoint::new(x, y)
}

fn effective_grip(window_width: u32, window_height: u32) -> u32 {
    PIN_RESIZE_GRIP
        .min((window_width / 3).max(1))
        .min((window_height / 3).max(1))
}

fn normalized_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale.clamp(PIN_MIN_SCALE, PIN_MAX_SCALE)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_window_size_basic() {
        let (w, h) = scaled_window_size(PixelSize::new(100, 50), 2.0);
        assert_eq!((w, h), (200, 100));
    }

    #[test]
    fn scaled_window_size_min_clamp() {
        let (w, h) = scaled_window_size(PixelSize::new(100, 50), 0.01);
        assert_eq!((w, h), (5, 3));
    }

    #[test]
    fn resize_hotspots_prefer_corners_then_edges() {
        let size = PixelSize::new(200, 120);
        assert_eq!(
            pin_resize_handle_at(PixelPoint::new(0, 0), size.width, size.height),
            Some(PinResizeHandle::NorthWest)
        );
        assert_eq!(
            pin_resize_handle_at(PixelPoint::new(199, 119), size.width, size.height),
            Some(PinResizeHandle::SouthEast)
        );
        assert_eq!(
            pin_resize_handle_at(PixelPoint::new(100, 0), size.width, size.height),
            Some(PinResizeHandle::North)
        );
        assert_eq!(
            pin_resize_handle_at(PixelPoint::new(100, 60), size.width, size.height),
            None
        );
    }

    #[test]
    fn resize_hotspots_do_not_overlap_on_small_windows() {
        assert_eq!(
            pin_resize_handle_at(PixelPoint::new(0, 2), 5, 5),
            Some(PinResizeHandle::West)
        );
        assert_eq!(pin_resize_handle_at(PixelPoint::new(2, 2), 5, 5), None);
        assert_eq!(
            pin_resize_handle_at(PixelPoint::new(4, 2), 5, 5),
            Some(PinResizeHandle::East)
        );
    }

    #[test]
    fn horizontal_resize_keeps_image_aspect_ratio() {
        let target = proportional_resize_target(
            PixelSize::new(400, 200),
            1.0,
            PixelSize::new(600, 205),
            PinResizeHandle::East,
        );

        assert_eq!(target.size, PixelSize::new(600, 300));
        assert!((target.scale - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn corner_resize_uses_the_axis_with_the_larger_scale_change() {
        let target = proportional_resize_target(
            PixelSize::new(400, 200),
            1.0,
            PixelSize::new(430, 400),
            PinResizeHandle::SouthEast,
        );

        assert_eq!(target.size, PixelSize::new(800, 400));
        assert!((target.scale - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn manual_west_resize_preserves_the_opposite_anchor() {
        let image = PixelSize::new(100, 50);
        let target = pin_resize_target_from_drag(
            image,
            1.0,
            PixelSize::new(100, 50),
            (0.0, 0.0),
            (-40.0, 0.0),
            PinResizeHandle::West,
        );
        let position = pin_resize_anchor_position(
            PinResizeHandle::West,
            PixelPoint::new(300, 200),
            PixelSize::new(100, 50),
            target.size,
        );

        assert_eq!(target.size, PixelSize::new(140, 70));
        assert_eq!(position, PixelPoint::new(260, 200));
    }

    #[test]
    fn resize_target_respects_the_existing_scale_limits() {
        let target = proportional_resize_target(
            PixelSize::new(100, 50),
            1.0,
            PixelSize::new(100_000, 100_000),
            PinResizeHandle::SouthEast,
        );

        assert_eq!(target.size, PixelSize::new(800, 400));
        assert!((target.scale - PIN_MAX_SCALE).abs() < f64::EPSILON);
    }

    #[test]
    fn fit_to_image_restores_the_original_pixel_size() {
        assert_eq!(
            fit_to_image_target(PixelSize::new(1_920, 1_080)),
            PinResizeTarget {
                scale: 1.0,
                size: PixelSize::new(1_920, 1_080),
            }
        );
    }
}
