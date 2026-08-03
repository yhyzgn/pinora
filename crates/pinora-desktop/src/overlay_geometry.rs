//! Overlay 的无窗口物理像素坐标映射与选区命中。
//!
//! 调用方负责将框架事件转换为物理像素。本模块不持有窗口、图形表面或交互状态。

use pinora_core::{PixelPoint, PixelRect, PixelSize, SelectionHandle};

/// 选区调整手柄的物理像素命中半径。
pub const SELECTION_HANDLE_HIT_RADIUS: i32 = 7;

/// 只有空标注文档且不存在草稿时，已确认选区才允许调整尺寸。
pub const fn selection_resize_allowed(document_is_empty: bool, has_draft: bool) -> bool {
    document_is_empty && !has_draft
}

/// 返回距离给定物理像素点最近的选区手柄。
///
/// 多个手柄命中时保留 `SelectionHandle::ALL` 的稳定枚举顺序，因而窄选区的角优先级
/// 与原 Overlay 行为保持一致。
pub fn selection_handle_at(rect: PixelRect, point: PixelPoint) -> Option<SelectionHandle> {
    SelectionHandle::ALL
        .into_iter()
        .filter_map(|handle| {
            let center = handle.center(rect);
            let dx = (i64::from(point.x) - i64::from(center.x)).abs();
            let dy = (i64::from(point.y) - i64::from(center.y)).abs();
            (dx <= i64::from(SELECTION_HANDLE_HIT_RADIUS)
                && dy <= i64::from(SELECTION_HANDLE_HIT_RADIUS))
            .then_some((
                handle,
                dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)),
            ))
        })
        .min_by_key(|(_, distance_squared)| *distance_squared)
        .map(|(handle, _)| handle)
}

/// 将缓冲区选区映射到原始截图像素。右、下边缘向上取整，避免缩放后丢失源像素。
pub fn buffer_rect_to_source(
    display_rect: PixelRect,
    buffer_size: PixelSize,
    source_size: PixelSize,
) -> PixelRect {
    let buffer_width = i64::from(buffer_size.width.max(1));
    let buffer_height = i64::from(buffer_size.height.max(1));
    let source_width = i64::from(source_size.width.max(1));
    let source_height = i64::from(source_size.height.max(1));
    let x0 =
        (i64::from(display_rect.origin.x) * source_width / buffer_width).clamp(0, source_width - 1);
    let y0 = (i64::from(display_rect.origin.y) * source_height / buffer_height)
        .clamp(0, source_height - 1);
    let x1 = ((i64::from(display_rect.right()) * source_width + buffer_width - 1) / buffer_width)
        .clamp(x0 + 1, source_width);
    let y1 = ((i64::from(display_rect.bottom()) * source_height + buffer_height - 1)
        / buffer_height)
        .clamp(y0 + 1, source_height);
    PixelRect::new(x0 as i32, y0 as i32, (x1 - x0) as u32, (y1 - y0) as u32)
}

/// 将缓冲区光标映射到原图选区的局部标注坐标。
///
/// 普通绘制与取色要求光标落在显示选区内。已选标注的拖动可以显式允许越界，以便
/// 保持既有“可将对象拖出画布”的预览语义。
pub fn selection_to_annotation_local(
    display_selection: PixelRect,
    source_selection: PixelRect,
    buffer_cursor: PixelPoint,
    require_inside_selection: bool,
) -> Option<PixelPoint> {
    if require_inside_selection && !display_selection.contains_point(buffer_cursor) {
        return None;
    }
    let local_x = buffer_cursor.x.saturating_sub(display_selection.origin.x);
    let local_y = buffer_cursor.y.saturating_sub(display_selection.origin.y);
    let display_width = i64::from(display_selection.size.width.max(1));
    let display_height = i64::from(display_selection.size.height.max(1));
    let source_width = i64::from(source_selection.size.width.max(1));
    let source_height = i64::from(source_selection.size.height.max(1));
    Some(PixelPoint::new(
        (i64::from(local_x) * source_width / display_width) as i32,
        (i64::from(local_y) * source_height / display_height) as i32,
    ))
}

/// 将窗口客户区中的物理鼠标点映射到图像像素，并夹紧到图像边界。
pub fn window_point_to_image(
    point: (f64, f64),
    window_size: PixelSize,
    image_size: PixelSize,
) -> PixelPoint {
    let image_x = if window_size.width == 0 {
        0
    } else {
        ((point.0 * f64::from(image_size.width)) / f64::from(window_size.width)).round() as i32
    };
    let image_y = if window_size.height == 0 {
        0
    } else {
        ((point.1 * f64::from(image_size.height)) / f64::from(window_size.height)).round() as i32
    };
    PixelPoint::new(
        image_x.clamp(0, maximum_image_coordinate(image_size.width)),
        image_y.clamp(0, maximum_image_coordinate(image_size.height)),
    )
}

/// 将窗口客户区中任意顺序的两个物理点转换为图像选区。
pub fn window_selection_to_image(
    start: (f64, f64),
    end: (f64, f64),
    window_size: PixelSize,
    image_size: PixelSize,
) -> PixelRect {
    let x0 = window_edge_to_image(start.0, window_size.width, image_size.width);
    let y0 = window_edge_to_image(start.1, window_size.height, image_size.height);
    let x1 = window_edge_to_image(end.0, window_size.width, image_size.width);
    let y1 = window_edge_to_image(end.1, window_size.height, image_size.height);
    PixelRect::new(x0.min(x1), y0.min(y1), x0.abs_diff(x1), y0.abs_diff(y1))
}

/// 返回两个窗口客户区点围成的非负物理像素矩形。
pub fn window_rect_from_points(start: (f64, f64), end: (f64, f64)) -> PixelRect {
    let x0 = start.0.min(end.0).max(0.0).round() as i32;
    let y0 = start.1.min(end.1).max(0.0).round() as i32;
    let x1 = start.0.max(end.0).max(0.0).round() as i32;
    let y1 = start.1.max(end.1).max(0.0).round() as i32;
    PixelRect::new(x0, y0, (x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32)
}

fn window_edge_to_image(value: f64, window_extent: u32, image_extent: u32) -> i32 {
    if window_extent == 0 || image_extent == 0 {
        return 0;
    }
    let clamped = value.clamp(0.0, f64::from(window_extent));
    ((clamped / f64::from(window_extent)) * f64::from(image_extent)).round() as i32
}

fn maximum_image_coordinate(extent: u32) -> i32 {
    i32::try_from(extent.saturating_sub(1)).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_to_source_preserves_identity_and_scales_the_full_frame() {
        let identity = buffer_rect_to_source(
            PixelRect::new(10, 20, 100, 50),
            PixelSize::new(3840, 2160),
            PixelSize::new(3840, 2160),
        );
        assert_eq!(identity, PixelRect::new(10, 20, 100, 50));

        let full = buffer_rect_to_source(
            PixelRect::new(0, 0, 1920, 1080),
            PixelSize::new(1920, 1080),
            PixelSize::new(3840, 2160),
        );
        assert_eq!(full, PixelRect::new(0, 0, 3840, 2160));

        let zero_sized = buffer_rect_to_source(
            PixelRect::new(5, 5, 3, 3),
            PixelSize::new(0, 0),
            PixelSize::new(0, 0),
        );
        assert_eq!(zero_sized, PixelRect::new(0, 0, 1, 1));
    }

    #[test]
    fn selection_local_mapping_rejects_drawing_outside_but_allows_drag_preview() {
        let display = PixelRect::new(100, 50, 200, 100);
        let source = PixelRect::new(0, 0, 400, 200);
        assert_eq!(
            selection_to_annotation_local(display, source, PixelPoint::new(150, 75), true),
            Some(PixelPoint::new(100, 50))
        );
        assert_eq!(
            selection_to_annotation_local(display, source, PixelPoint::new(90, 40), true),
            None
        );
        assert_eq!(
            selection_to_annotation_local(display, source, PixelPoint::new(90, 40), false),
            Some(PixelPoint::new(-20, -20))
        );
    }

    #[test]
    fn window_mapping_handles_reversed_points_and_zero_window_extent() {
        assert_eq!(
            window_selection_to_image(
                (20.0, 10.0),
                (120.0, 60.0),
                PixelSize::new(200, 100),
                PixelSize::new(100, 50),
            ),
            PixelRect::new(10, 5, 50, 25)
        );
        assert_eq!(
            window_selection_to_image(
                (120.0, 60.0),
                (20.0, 10.0),
                PixelSize::new(200, 100),
                PixelSize::new(100, 50),
            ),
            PixelRect::new(10, 5, 50, 25)
        );
        assert_eq!(
            window_point_to_image((20.0, 10.0), PixelSize::new(0, 0), PixelSize::new(8, 6)),
            PixelPoint::new(0, 0)
        );
        assert_eq!(
            window_point_to_image(
                (100.0, 100.0),
                PixelSize::new(100, 100),
                PixelSize::new(u32::MAX, u32::MAX),
            ),
            PixelPoint::new(i32::MAX, i32::MAX)
        );
        assert_eq!(
            window_rect_from_points((-5.0, 8.2), (10.1, -2.0)),
            PixelRect::new(0, 0, 10, 8)
        );
    }

    #[test]
    fn selection_handles_prefer_nearest_stable_corner_and_require_an_empty_document() {
        let rect = PixelRect::new(10, 20, 101, 101);
        assert_eq!(
            selection_handle_at(rect, PixelPoint::new(10, 20)),
            Some(SelectionHandle::NorthWest)
        );
        assert_eq!(
            selection_handle_at(rect, PixelPoint::new(60, 20)),
            Some(SelectionHandle::North)
        );
        assert_eq!(
            selection_handle_at(rect, PixelPoint::new(110, 120)),
            Some(SelectionHandle::SouthEast)
        );
        assert_eq!(
            selection_handle_at(PixelRect::new(0, 0, 3, 9), PixelPoint::new(2, 4)),
            Some(SelectionHandle::East)
        );
        assert_eq!(selection_handle_at(rect, PixelPoint::new(60, 70)), None);
        assert!(selection_resize_allowed(true, false));
        assert!(!selection_resize_allowed(false, false));
        assert!(!selection_resize_allowed(true, true));
    }
}
