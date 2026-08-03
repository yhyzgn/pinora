//! 无窗口 XRGB 栅格化和贴图基础帧缓存。
//!
//! 这里的函数只处理调用方拥有的像素缓冲，既不创建窗口也不上传到图形表面。

use pinora_core::{ErrorCode, PinoraError, PixelPoint, PixelRect, PixelSize, SelectionHandle};

/// Overlay 选区手柄的半径（物理像素）。
pub const XRGB_SELECTION_HANDLE_RENDER_RADIUS: i32 = 4;

/// 贴图基础帧缓存：不包含 OCR、拖选或锁定边框等每帧变化的叠加层。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinRenderCache {
    width: u32,
    height: u32,
    opacity_factor: u32,
    pixels: Vec<u32>,
}

impl PinRenderCache {
    pub fn matches(&self, width: u32, height: u32, opacity: f64) -> bool {
        self.width == width
            && self.height == height
            && self.opacity_factor == opacity_factor(opacity)
    }

    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }
}

/// 以最近邻缩放与固定压暗规则构建贴图基础帧。
pub fn build_pin_render_cache(
    source: &[u32],
    source_size: PixelSize,
    target_size: PixelSize,
    opacity: f64,
) -> Result<PinRenderCache, PinoraError> {
    require_non_zero_size(source_size, "pin source")?;
    require_non_zero_size(target_size, "pin render target")?;
    let source_len = pixel_count(source_size, "pin source")?;
    if source.len() != source_len {
        return Err(PinoraError::new(
            ErrorCode::InvalidState,
            "pin source pixels do not match image dimensions",
        ));
    }
    let target_len = pixel_count(target_size, "pin render target")?;
    let source_width = usize::try_from(source_size.width)
        .map_err(|_| PinoraError::new(ErrorCode::InvalidState, "pin source is too large"))?;
    let source_height = usize::try_from(source_size.height)
        .map_err(|_| PinoraError::new(ErrorCode::InvalidState, "pin source is too large"))?;
    let width = usize::try_from(target_size.width)
        .map_err(|_| PinoraError::new(ErrorCode::InvalidState, "pin render target is too large"))?;
    let height = usize::try_from(target_size.height)
        .map_err(|_| PinoraError::new(ErrorCode::InvalidState, "pin render target is too large"))?;
    let mut pixels = vec![0; target_len];
    if source_width == width && source_height == height {
        pixels.copy_from_slice(source);
    } else {
        scale_xrgb_nearest(
            source,
            source_width,
            source_height,
            &mut pixels,
            width,
            height,
        );
    }
    apply_opacity_darken(&mut pixels, opacity);
    Ok(PinRenderCache {
        width: target_size.width,
        height: target_size.height,
        opacity_factor: opacity_factor(opacity),
        pixels,
    })
}

/// 返回完整 XRGB 帧所需的像素数量，并将尺寸溢出映射为受控错误。
pub fn xrgb_pixel_count(size: PixelSize) -> Result<usize, PinoraError> {
    pixel_count(size, "xrgb frame")
}

/// 用源缓冲中指定的矩形恢复目标缓冲，矩形会裁剪到两者公共边界内。
pub fn blit_xrgb_rect(
    destination: &mut [u32],
    source: &[u32],
    stride: usize,
    height: usize,
    rect: PixelRect,
) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right() as usize).min(stride);
    let y1 = (rect.bottom() as usize).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let row_width = x1 - x0;
    for y in y0..y1 {
        let start = y * stride + x0;
        destination[start..start + row_width].copy_from_slice(&source[start..start + row_width]);
    }
}

/// 最近邻缩放完整 XRGB 缓冲。调用方必须提供非零且长度匹配的尺寸。
pub fn scale_xrgb_nearest(
    source: &[u32],
    source_width: usize,
    source_height: usize,
    destination: &mut [u32],
    destination_width: usize,
    destination_height: usize,
) {
    for y in 0..destination_height {
        let source_y = y * source_height / destination_height;
        let source_row = source_y * source_width;
        let destination_row = y * destination_width;
        for x in 0..destination_width {
            destination[destination_row + x] =
                source[source_row + x * source_width / destination_width];
        }
    }
}

/// 绘制两像素宽的选区边框。
pub fn draw_xrgb_rect_border(
    buffer: &mut [u32],
    stride: usize,
    height: usize,
    rect: PixelRect,
    color: u32,
) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right() as usize).min(stride);
    let y1 = (rect.bottom() as usize).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for thickness in 0..2usize {
        let top = y0 + thickness;
        let bottom = y1.saturating_sub(1 + thickness);
        if top < height {
            for x in x0..x1 {
                buffer[top * stride + x] = color;
            }
        }
        if bottom < height && bottom >= y0 {
            for x in x0..x1 {
                buffer[bottom * stride + x] = color;
            }
        }
        let left = x0 + thickness;
        let right = x1.saturating_sub(1 + thickness);
        for y in y0..y1 {
            if left < stride {
                buffer[y * stride + left] = color;
            }
            if right < stride {
                buffer[y * stride + right] = color;
            }
        }
    }
}

/// 绘制八个选区调整手柄。
pub fn draw_xrgb_selection_handles(
    buffer: &mut [u32],
    stride: usize,
    height: usize,
    rect: PixelRect,
) {
    for handle in SelectionHandle::ALL {
        let center = handle.center(rect);
        fill_xrgb_rect(
            buffer,
            stride,
            height,
            PixelRect::new(
                center.x.saturating_sub(XRGB_SELECTION_HANDLE_RENDER_RADIUS),
                center.y.saturating_sub(XRGB_SELECTION_HANDLE_RENDER_RADIUS),
                (XRGB_SELECTION_HANDLE_RENDER_RADIUS * 2 + 1) as u32,
                (XRGB_SELECTION_HANDLE_RENDER_RADIUS * 2 + 1) as u32,
            ),
            0x00_23_29_33,
        );
        fill_xrgb_rect(
            buffer,
            stride,
            height,
            PixelRect::new(
                center
                    .x
                    .saturating_sub(XRGB_SELECTION_HANDLE_RENDER_RADIUS - 1),
                center
                    .y
                    .saturating_sub(XRGB_SELECTION_HANDLE_RENDER_RADIUS - 1),
                (XRGB_SELECTION_HANDLE_RENDER_RADIUS * 2 - 1) as u32,
                (XRGB_SELECTION_HANDLE_RENDER_RADIUS * 2 - 1) as u32,
            ),
            0x00_FF_FF_FF,
        );
    }
}

/// 在 XRGB 缓冲上绘制轴对齐轮廓，用于 OCR 词框和拖选反馈。
pub fn draw_xrgb_outline(
    buffer: &mut [u32],
    stride: usize,
    height: usize,
    from: PixelPoint,
    to: PixelPoint,
    color: u32,
) {
    if stride == 0 || height == 0 {
        return;
    }
    let x0 = from.x.clamp(0, stride as i32 - 1) as usize;
    let x1 = to.x.clamp(0, stride as i32 - 1) as usize;
    let y0 = from.y.clamp(0, height as i32 - 1) as usize;
    let y1 = to.y.clamp(0, height as i32 - 1) as usize;
    if x1 < x0 || y1 < y0 {
        return;
    }
    for x in x0..=x1 {
        buffer[y0 * stride + x] = color;
        buffer[y1 * stride + x] = color;
    }
    for y in y0..=y1 {
        buffer[y * stride + x0] = color;
        buffer[y * stride + x1] = color;
    }
}

/// 绘制完整缓冲的单像素外边框。
pub fn draw_xrgb_border(buffer: &mut [u32], width: usize, height: usize, color: u32) {
    if width == 0 || height == 0 {
        return;
    }
    for x in 0..width {
        buffer[x] = color;
        buffer[(height - 1) * width + x] = color;
    }
    for y in 0..height {
        buffer[y * width] = color;
        buffer[y * width + width - 1] = color;
    }
}

fn require_non_zero_size(size: PixelSize, subject: &str) -> Result<(), PinoraError> {
    if size.width == 0 || size.height == 0 {
        return Err(PinoraError::new(
            ErrorCode::InvalidState,
            format!("{subject} dimensions must be non-zero"),
        ));
    }
    Ok(())
}

fn pixel_count(size: PixelSize, subject: &str) -> Result<usize, PinoraError> {
    let width = usize::try_from(size.width).map_err(|_| {
        PinoraError::new(ErrorCode::InvalidState, format!("{subject} is too large"))
    })?;
    let height = usize::try_from(size.height).map_err(|_| {
        PinoraError::new(ErrorCode::InvalidState, format!("{subject} is too large"))
    })?;
    width
        .checked_mul(height)
        .ok_or_else(|| PinoraError::new(ErrorCode::InvalidState, format!("{subject} is too large")))
}

/// 无窗口透明时，用压暗模拟 opacity（1.0 = 原色，0.15 = 很暗）。
fn apply_opacity_darken(buffer: &mut [u32], opacity: f64) {
    let factor = opacity_factor(opacity);
    if factor == 256 {
        return;
    }
    for pixel in buffer {
        let red = ((*pixel >> 16) & 0xff) * factor / 256;
        let green = ((*pixel >> 8) & 0xff) * factor / 256;
        let blue = (*pixel & 0xff) * factor / 256;
        *pixel = (red << 16) | (green << 8) | blue;
    }
}

fn opacity_factor(opacity: f64) -> u32 {
    if opacity >= 0.999 {
        256
    } else {
        (opacity.clamp(0.05, 1.0) * 256.0) as u32
    }
}

fn fill_xrgb_rect(buffer: &mut [u32], stride: usize, height: usize, rect: PixelRect, color: u32) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right() as usize).min(stride);
    let y1 = (rect.bottom() as usize).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for y in y0..y1 {
        buffer[y * stride + x0..y * stride + x1].fill(color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_render_cache_scales_and_darkens_for_its_exact_key() {
        let cache = build_pin_render_cache(
            &[0x00ff_0000, 0x0000_00ff],
            PixelSize::new(2, 1),
            PixelSize::new(4, 1),
            0.5,
        )
        .expect("cache");

        assert_eq!(
            cache.pixels(),
            [0x007f_0000, 0x007f_0000, 0x0000_007f, 0x0000_007f]
        );
        assert!(cache.matches(4, 1, 0.5));
        assert!(!cache.matches(2, 1, 0.5));
        assert!(!cache.matches(4, 1, 0.75));
    }

    #[test]
    fn near_opaque_pin_render_cache_keeps_existing_pixel_semantics() {
        let cache = build_pin_render_cache(
            &[0x0011_2233],
            PixelSize::new(1, 1),
            PixelSize::new(1, 1),
            0.999,
        )
        .expect("cache");

        assert_eq!(cache.pixels(), [0x0011_2233]);
        assert!(cache.matches(1, 1, 1.0));
    }

    #[test]
    fn invalid_pin_cache_dimensions_are_rejected_without_indexing_pixels() {
        let error = build_pin_render_cache(&[], PixelSize::new(0, 1), PixelSize::new(1, 1), 1.0)
            .expect_err("zero source width must be rejected");

        assert_eq!(error.code, ErrorCode::InvalidState);
    }

    #[test]
    fn xrgb_pixel_count_matches_the_frame_dimensions() {
        assert_eq!(
            xrgb_pixel_count(PixelSize::new(3, 2)).expect("pixel count"),
            6
        );
    }

    #[test]
    fn blit_and_outline_are_bounded_to_the_xrgb_frame() {
        let source = vec![1, 2, 3, 4];
        let mut frame = vec![0; 4];
        blit_xrgb_rect(&mut frame, &source, 2, 2, PixelRect::new(-1, -1, 3, 3));
        assert_eq!(frame, source);

        draw_xrgb_outline(
            &mut frame,
            2,
            2,
            PixelPoint::new(-20, -20),
            PixelPoint::new(20, 20),
            9,
        );
        assert_eq!(frame, vec![9; 4]);
    }

    #[test]
    fn selection_handles_use_the_stable_radius_and_remain_clipped() {
        let mut frame = vec![0; 16];
        draw_xrgb_selection_handles(&mut frame, 4, 4, PixelRect::new(0, 0, 4, 4));

        assert_eq!(XRGB_SELECTION_HANDLE_RENDER_RADIUS, 4);
        assert!(
            frame
                .iter()
                .all(|pixel| matches!(*pixel, 0 | 0x00_23_29_33 | 0x00_FF_FF_FF))
        );
        assert!(frame.contains(&0x00_FF_FF_FF));
    }
}
