//! 当前 Overlay 选区的物理像素读数。
//!
//! 读数只描述已经映射到源图的矩形及其全局原点；布局和绘制都限制在现有 Overlay
//! 帧内，绝不创建临时窗口或请求平台能力。

use crate::settings_panel::draw_text;
use pinora_core::{PixelPoint, PixelRect};

const PANEL_HEIGHT: u32 = 19;
const PANEL_HORIZONTAL_PADDING: u32 = 6;
const PANEL_MARGIN: i32 = 9;
const PANEL_EDGE_GAP: i32 = 5;
const TEXT_Y_OFFSET: i32 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionReadout {
    source_rect: PixelRect,
    global_origin: PixelPoint,
}

impl SelectionReadout {
    pub(crate) fn new(source_rect: PixelRect, display_origin: PixelPoint) -> Self {
        Self {
            source_rect,
            global_origin: PixelPoint::new(
                display_origin.x.saturating_add(source_rect.origin.x),
                display_origin.y.saturating_add(source_rect.origin.y),
            ),
        }
    }

    pub(crate) fn text(self) -> String {
        format!(
            "W{} H{} X{} Y{}",
            self.source_rect.size.width,
            self.source_rect.size.height,
            self.global_origin.x,
            self.global_origin.y
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionReadoutLayout {
    pub(crate) panel: PixelRect,
    text_origin: PixelPoint,
}

/// 在选区外优先放置读数；工具栏占用下方时优先切换到上方。
pub(crate) fn layout_selection_readout(
    selection: PixelRect,
    image_width: u32,
    image_height: u32,
    toolbar: Option<PixelRect>,
    text: &str,
) -> SelectionReadoutLayout {
    let image = PixelRect::new(0, 0, image_width.max(1), image_height.max(1));
    let requested_width = u32::try_from(text.chars().count())
        .unwrap_or(u32::MAX)
        .saturating_mul(6)
        .saturating_add(PANEL_HORIZONTAL_PADDING.saturating_mul(2));
    let panel_width = requested_width.clamp(1, image.size.width);
    let panel_height = PANEL_HEIGHT.min(image.size.height).max(1);
    let x = selection
        .origin
        .x
        .saturating_add((selection.size.width as i32 - panel_width as i32) / 2)
        .clamp(0, image.size.width.saturating_sub(panel_width) as i32);

    let candidates = [
        selection
            .origin
            .y
            .saturating_sub(PANEL_MARGIN)
            .saturating_sub(panel_height as i32),
        selection.bottom().saturating_add(PANEL_MARGIN),
        selection.origin.y.saturating_add(PANEL_EDGE_GAP),
        0,
        image.size.height.saturating_sub(panel_height) as i32,
    ];
    let panel = candidates
        .into_iter()
        .map(|y| PixelRect::new(x, y, panel_width, panel_height))
        .filter(|candidate| {
            image.contains_point(candidate.origin)
                && candidate.right() <= image.right()
                && candidate.bottom() <= image.bottom()
        })
        .find(|candidate| toolbar.is_none_or(|bounds| !rects_overlap(*candidate, bounds)))
        .unwrap_or_else(|| PixelRect::new(x, 0, panel_width, panel_height));

    SelectionReadoutLayout {
        panel,
        text_origin: PixelPoint::new(
            panel
                .origin
                .x
                .saturating_add(PANEL_HORIZONTAL_PADDING as i32),
            panel.origin.y.saturating_add(TEXT_Y_OFFSET),
        ),
    }
}

pub(crate) fn paint_selection_readout(
    frame: &mut [u32],
    stride: usize,
    height: usize,
    layout: SelectionReadoutLayout,
    text: &str,
) {
    fill_rect(frame, stride, height, layout.panel, 0x00_1A_1F_28);
    draw_outline(frame, stride, height, layout.panel, 0x00_FF_CC_33);
    let max_chars = layout
        .panel
        .size
        .width
        .saturating_sub(PANEL_HORIZONTAL_PADDING.saturating_mul(2))
        / 6;
    let visible_text: String = text
        .chars()
        .take(usize::try_from(max_chars).unwrap_or(usize::MAX))
        .collect();
    draw_text(
        frame,
        stride,
        height,
        layout.text_origin.x,
        layout.text_origin.y,
        &visible_text,
        0x00_F5_F7_FA,
    );
}

fn rects_overlap(left: PixelRect, right: PixelRect) -> bool {
    left.origin.x < right.right()
        && right.origin.x < left.right()
        && left.origin.y < right.bottom()
        && right.origin.y < left.bottom()
}

fn fill_rect(frame: &mut [u32], stride: usize, height: usize, rect: PixelRect, color: u32) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right().max(0) as usize).min(stride);
    let y1 = (rect.bottom().max(0) as usize).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for y in y0..y1 {
        frame[y * stride + x0..y * stride + x1].fill(color);
    }
}

fn draw_outline(frame: &mut [u32], stride: usize, height: usize, rect: PixelRect, color: u32) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right().saturating_sub(1).max(0) as usize).min(stride.saturating_sub(1));
    let y1 = (rect.bottom().saturating_sub(1).max(0) as usize).min(height.saturating_sub(1));
    if x1 < x0 || y1 < y0 || x0 >= stride || y0 >= height {
        return;
    }
    for x in x0..=x1 {
        frame[y0 * stride + x] = color;
        frame[y1 * stride + x] = color;
    }
    for y in y0..=y1 {
        frame[y * stride + x0] = color;
        frame[y * stride + x1] = color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readout_uses_source_pixels_and_preserves_negative_global_origin() {
        let readout = SelectionReadout::new(
            PixelRect::new(32, 18, 1920, 1080),
            PixelPoint::new(-2560, 120),
        );

        assert_eq!(readout.text(), "W1920 H1080 X-2528 Y138");
    }

    #[test]
    fn layout_prefers_space_above_the_selection_when_toolbar_is_below() {
        let selection = PixelRect::new(150, 110, 180, 90);
        let toolbar = PixelRect::new(120, 210, 240, 42);
        let layout =
            layout_selection_readout(selection, 500, 400, Some(toolbar), "W180 H90 X150 Y110");

        assert!(layout.panel.bottom() <= selection.origin.y);
        assert!(!rects_overlap(layout.panel, toolbar));
    }

    #[test]
    fn layout_remains_inside_a_tiny_overlay_without_an_auxiliary_surface() {
        let layout =
            layout_selection_readout(PixelRect::new(0, 0, 2, 2), 2, 2, None, "W2 H2 X0 Y0");

        assert_eq!(layout.panel, PixelRect::new(0, 0, 2, 2));
    }

    #[test]
    fn readout_paint_is_bounded_to_its_panel() {
        let layout = layout_selection_readout(
            PixelRect::new(10, 30, 40, 20),
            80,
            80,
            None,
            "W40 H20 X10 Y30",
        );
        let mut frame = vec![0u32; 80 * 80];
        paint_selection_readout(&mut frame, 80, 80, layout, "W40 H20 X10 Y30 EXTRA TEXT");

        assert!(frame.iter().any(|pixel| *pixel != 0));
        for y in 0..80 {
            for x in 0..80 {
                if !layout.panel.contains_point(PixelPoint::new(x, y)) {
                    assert_eq!(frame[y as usize * 80 + x as usize], 0, "at ({x}, {y})");
                }
            }
        }
    }
}
