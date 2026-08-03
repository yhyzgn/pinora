//! Overlay 标注选中框的显示投影与脏区裁剪。

use pinora_core::{PixelRect, PixelSize};

/// 将原图选区局部坐标的标注框投影到显示选区。
///
/// 右、下边缘向上取整，且投影后的尺寸至少为一个物理像素，确保细小标注仍有可见
/// 选择框。越界源框先裁剪到原图选区。
pub fn annotation_bounds_to_display(
    annotation_bounds: PixelRect,
    source_size: PixelSize,
    display_rect: PixelRect,
) -> Option<PixelRect> {
    if source_size.is_empty() || display_rect.size.is_empty() {
        return None;
    }
    let local =
        annotation_bounds.clamp_to(PixelRect::new(0, 0, source_size.width, source_size.height))?;
    let source_width = u64::from(source_size.width);
    let source_height = u64::from(source_size.height);
    let display_width = u64::from(display_rect.size.width);
    let display_height = u64::from(display_rect.size.height);
    let x0 = u64::try_from(local.origin.x).ok()? * display_width / source_width;
    let y0 = u64::try_from(local.origin.y).ok()? * display_height / source_height;
    let x1 = (u64::try_from(local.right()).ok()? * display_width).div_ceil(source_width);
    let y1 = (u64::try_from(local.bottom()).ok()? * display_height).div_ceil(source_height);
    let width = capped_i32(x1).saturating_sub(capped_i32(x0)).max(1) as u32;
    let height = capped_i32(y1).saturating_sub(capped_i32(y0)).max(1) as u32;
    Some(PixelRect::new(
        display_rect.origin.x.saturating_add(capped_i32(x0)),
        display_rect.origin.y.saturating_add(capped_i32(y0)),
        width,
        height,
    ))
}

/// 将脏区向四周扩张并裁剪到完整显示缓冲区。
pub fn expand_damage_rect(rect: PixelRect, padding: i32, bounds: PixelSize) -> PixelRect {
    let max_x = capped_i32(u64::from(bounds.width));
    let max_y = capped_i32(u64::from(bounds.height));
    let x0 = rect.origin.x.saturating_sub(padding).max(0);
    let y0 = rect.origin.y.saturating_sub(padding).max(0);
    let x1 = rect.right().saturating_add(padding).clamp(0, max_x);
    let y1 = rect.bottom().saturating_add(padding).clamp(0, max_y);
    PixelRect::new(
        x0.min(max_x),
        y0.min(max_y),
        x1.saturating_sub(x0.min(max_x)).max(0) as u32,
        y1.saturating_sub(y0.min(max_y)).max(0) as u32,
    )
}

fn capped_i32(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_projection_scales_and_preserves_a_one_pixel_selection_border() {
        assert_eq!(
            annotation_bounds_to_display(
                PixelRect::new(100, 50, 100, 50),
                PixelSize::new(400, 200),
                PixelRect::new(10, 20, 200, 100),
            ),
            Some(PixelRect::new(60, 45, 50, 25))
        );
        assert_eq!(
            annotation_bounds_to_display(
                PixelRect::new(399, 199, 1, 1),
                PixelSize::new(400, 200),
                PixelRect::new(0, 0, 3, 2),
            ),
            Some(PixelRect::new(2, 1, 1, 1))
        );
    }

    #[test]
    fn annotation_projection_clips_or_rejects_invalid_source_space() {
        assert_eq!(
            annotation_bounds_to_display(
                PixelRect::new(-10, -10, 30, 30),
                PixelSize::new(20, 20),
                PixelRect::new(0, 0, 40, 40),
            ),
            Some(PixelRect::new(0, 0, 40, 40))
        );
        assert_eq!(
            annotation_bounds_to_display(
                PixelRect::new(0, 0, 1, 1),
                PixelSize::new(0, 20),
                PixelRect::new(0, 0, 40, 40),
            ),
            None
        );
    }

    #[test]
    fn expanded_damage_never_escapes_or_overflows_its_buffer() {
        assert_eq!(
            expand_damage_rect(PixelRect::new(2, 3, 4, 5), 3, PixelSize::new(10, 10)),
            PixelRect::new(0, 0, 9, 10)
        );
        assert_eq!(
            expand_damage_rect(
                PixelRect::new(i32::MAX - 2, i32::MAX - 2, 2, 2),
                i32::MAX,
                PixelSize::new(10, 10),
            ),
            PixelRect::new(0, 0, 10, 10)
        );
    }
}
