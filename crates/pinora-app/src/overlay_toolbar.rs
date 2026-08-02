//! 选区 Overlay 浮动工具栏：布局、命中、绘制（图像坐标系）。

use pinora_core::{AnnotateTool, PixelPoint, PixelRect};

/// 工具栏动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    Copy,
    Pin,
    Save,
    Ocr,
    Tool(AnnotateTool),
}

/// 单个按钮（图像像素坐标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolbarButton {
    pub action: ToolbarAction,
    pub label: &'static str,
    pub rect: PixelRect,
}

const BTN_W: u32 = 64;
const BTN_H: u32 = 36;
const GAP: u32 = 6;
const PAD: u32 = 8;
const BAR_MARGIN: i32 = 10;
/// 命中测试外扩，降低点不中概率。
const HIT_PAD: i32 = 4;

/// 在选区下方（空间不足则上方）排布工具栏。
pub fn layout_toolbar(selection: PixelRect, img_w: u32, img_h: u32) -> Vec<ToolbarButton> {
    let specs: &[(ToolbarAction, &str)] = &[
        (ToolbarAction::Copy, "复制"),
        (ToolbarAction::Pin, "贴图"),
        (ToolbarAction::Save, "保存"),
        (ToolbarAction::Ocr, "OCR"),
        (ToolbarAction::Tool(AnnotateTool::Rect), "矩形"),
        (ToolbarAction::Tool(AnnotateTool::Line), "直线"),
        (ToolbarAction::Tool(AnnotateTool::Arrow), "箭头"),
        (ToolbarAction::Tool(AnnotateTool::Pen), "画笔"),
        (ToolbarAction::Tool(AnnotateTool::Ellipse), "椭圆"),
        (ToolbarAction::Tool(AnnotateTool::Number), "序号"),
        (ToolbarAction::Tool(AnnotateTool::Mosaic), "马赛克"),
        (ToolbarAction::Tool(AnnotateTool::Text), "文本"),
        (ToolbarAction::Tool(AnnotateTool::ColorPicker), "取色"),
    ];
    let n = specs.len() as u32;
    if img_w < BTN_W + PAD * 2 || img_h < BTN_H + PAD * 2 {
        return Vec::new();
    }
    let available_width = img_w.saturating_sub(PAD * 2).max(1);
    let columns = ((available_width + GAP) / (BTN_W + GAP)).max(1).min(n);
    let rows = n.div_ceil(columns);
    let bar_w = PAD * 2 + columns * BTN_W + columns.saturating_sub(1) * GAP;
    let bar_h = PAD * 2 + rows * BTN_H + rows.saturating_sub(1) * GAP;
    if bar_h > img_h {
        return Vec::new();
    }

    let mut x = selection.origin.x + (selection.size.width as i32 - bar_w as i32) / 2;
    x = x.clamp(0, (img_w as i32 - bar_w as i32).max(0));

    // 优先选区下方
    let below_y = selection.bottom() + BAR_MARGIN;
    let above_y = selection.origin.y - bar_h as i32 - BAR_MARGIN;
    let y = if below_y + bar_h as i32 <= img_h as i32 {
        below_y
    } else if above_y >= 0 {
        above_y
    } else {
        (selection.bottom() - bar_h as i32 / 2).clamp(0, (img_h as i32 - bar_h as i32).max(0))
    };

    let mut buttons = Vec::with_capacity(specs.len());
    for (index, (action, label)) in specs.iter().enumerate() {
        let index = index as u32;
        let column = index % columns;
        let row = index / columns;
        let bx = x + PAD as i32 + (column * (BTN_W + GAP)) as i32;
        let by = y + PAD as i32 + (row * (BTN_H + GAP)) as i32;
        buttons.push(ToolbarButton {
            action: *action,
            label,
            rect: PixelRect::new(bx, by, BTN_W, BTN_H),
        });
    }
    buttons
}

/// 命中测试（带外扩）。
pub fn hit_test(buttons: &[ToolbarButton], p: PixelPoint) -> Option<ToolbarAction> {
    buttons.iter().find_map(|b| {
        let r = b.rect;
        let x0 = r.origin.x - HIT_PAD;
        let y0 = r.origin.y - HIT_PAD;
        let x1 = r.right() + HIT_PAD;
        let y1 = r.bottom() + HIT_PAD;
        if p.x >= x0 && p.y >= y0 && p.x < x1 && p.y < y1 {
            Some(b.action)
        } else {
            None
        }
    })
}

/// 工具栏总包围盒（用于避免点到工具栏时开始新选区）。
pub fn toolbar_bounds(buttons: &[ToolbarButton]) -> Option<PixelRect> {
    if buttons.is_empty() {
        return None;
    }
    let mut x0 = buttons[0].rect.origin.x;
    let mut y0 = buttons[0].rect.origin.y;
    let mut x1 = buttons[0].rect.right();
    let mut y1 = buttons[0].rect.bottom();
    for b in buttons.iter().skip(1) {
        x0 = x0.min(b.rect.origin.x);
        y0 = y0.min(b.rect.origin.y);
        x1 = x1.max(b.rect.right());
        y1 = y1.max(b.rect.bottom());
    }
    // 含内边距一点容差
    Some(PixelRect::new(
        x0 - PAD as i32,
        y0 - PAD as i32,
        (x1 - x0 + PAD as i32 * 2).max(0) as u32,
        (y1 - y0 + PAD as i32 * 2).max(0) as u32,
    ))
}

/// 绘制工具栏到 XRGB 缓冲（图像分辨率）。
pub fn paint_toolbar(
    frame: &mut [u32],
    stride: usize,
    height: usize,
    buttons: &[ToolbarButton],
    active_tool: AnnotateTool,
    current_color: [u8; 4],
) {
    let Some(bounds) = toolbar_bounds(buttons) else {
        return;
    };
    fill_rect(frame, stride, height, bounds, 0x00_28_28_30);
    // 边框
    draw_rect_outline(frame, stride, height, bounds, 0x00_80_80_90);

    for b in buttons {
        let selected = matches!(b.action, ToolbarAction::Tool(t) if t == active_tool);
        let bg = if selected {
            0x00_2A_6A_E0
        } else {
            match b.action {
                ToolbarAction::Copy => 0x00_1E_7A_4A,
                ToolbarAction::Pin => 0x00_6A_3A_B0,
                ToolbarAction::Save => 0x00_7A_5A_10,
                ToolbarAction::Ocr => 0x00_7A_2A_2A,
                ToolbarAction::Tool(_) => 0x00_3A_3A_48,
            }
        };
        fill_rect(frame, stride, height, b.rect, bg);
        draw_rect_outline(frame, stride, height, b.rect, 0x00_C0_C0_D0);
        if b.action == ToolbarAction::Tool(AnnotateTool::ColorPicker) {
            draw_color_picker_icon(frame, stride, height, b.rect, current_color);
        } else {
            // 简易点阵文字（首字/英文）
            let mark = button_mark(b);
            draw_mark(frame, stride, height, b.rect, mark, 0x00_FF_FF_FF);
        }
    }
}

fn button_mark(b: &ToolbarButton) -> &'static str {
    match b.action {
        ToolbarAction::Copy => "Cpy",
        ToolbarAction::Pin => "Pin",
        ToolbarAction::Save => "Sav",
        ToolbarAction::Ocr => "OCR",
        ToolbarAction::Tool(AnnotateTool::Rect) => "R",
        ToolbarAction::Tool(AnnotateTool::Line) => "L",
        ToolbarAction::Tool(AnnotateTool::Arrow) => "A",
        ToolbarAction::Tool(AnnotateTool::Pen) => "P",
        ToolbarAction::Tool(AnnotateTool::Ellipse) => "E",
        ToolbarAction::Tool(AnnotateTool::Number) => "N",
        ToolbarAction::Tool(AnnotateTool::Mosaic) => "M",
        ToolbarAction::Tool(AnnotateTool::Text) => "T",
        ToolbarAction::Tool(AnnotateTool::ColorPicker) => "",
    }
}

/// 取色器使用简洁滴管和当前颜色色块，避免再依赖语言文字宽度。
fn draw_color_picker_icon(
    frame: &mut [u32],
    stride: usize,
    height: usize,
    rect: PixelRect,
    color: [u8; 4],
) {
    let center_x = rect.origin.x + rect.size.width as i32 / 2 - 4;
    let center_y = rect.origin.y + rect.size.height as i32 / 2 - 2;
    for step in 0..9 {
        set_pixel(
            frame,
            stride,
            height,
            center_x + step,
            center_y - step,
            0x00_FF_FF_FF,
        );
        set_pixel(
            frame,
            stride,
            height,
            center_x + step,
            center_y - step + 1,
            0x00_FF_FF_FF,
        );
    }
    let swatch = PixelRect::new(rect.right() - 15, rect.bottom() - 15, 10, 10);
    fill_rect(
        frame,
        stride,
        height,
        swatch,
        (u32::from(color[0]) << 16) | (u32::from(color[1]) << 8) | u32::from(color[2]),
    );
    draw_rect_outline(frame, stride, height, swatch, 0x00_FF_FF_FF);
}

fn set_pixel(frame: &mut [u32], stride: usize, height: usize, x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 || x as usize >= stride || y as usize >= height {
        return;
    }
    let index = y as usize * stride + x as usize;
    if let Some(pixel) = frame.get_mut(index) {
        *pixel = color;
    }
}

fn fill_rect(frame: &mut [u32], stride: usize, height: usize, rect: PixelRect, color: u32) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right() as usize).min(stride);
    let y1 = (rect.bottom() as usize).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for y in y0..y1 {
        let row = y * stride;
        for x in x0..x1 {
            if row + x < frame.len() {
                frame[row + x] = color;
            }
        }
    }
}

fn draw_rect_outline(frame: &mut [u32], stride: usize, height: usize, rect: PixelRect, color: u32) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right().saturating_sub(1) as usize).min(stride.saturating_sub(1));
    let y1 = (rect.bottom().saturating_sub(1) as usize).min(height.saturating_sub(1));
    if x1 < x0 || y1 < y0 {
        return;
    }
    for x in x0..=x1 {
        let top = y0 * stride + x;
        let bot = y1 * stride + x;
        if top < frame.len() {
            frame[top] = color;
        }
        if bot < frame.len() {
            frame[bot] = color;
        }
    }
    for y in y0..=y1 {
        let left = y * stride + x0;
        let right = y * stride + x1;
        if left < frame.len() {
            frame[left] = color;
        }
        if right < frame.len() {
            frame[right] = color;
        }
    }
}

/// 极简 3×5 点阵，画在按钮中央。
fn draw_mark(
    frame: &mut [u32],
    stride: usize,
    height: usize,
    rect: PixelRect,
    mark: &str,
    color: u32,
) {
    let glyphs: &[(char, [u8; 5])] = &[
        ('C', [0b111, 0b100, 0b100, 0b100, 0b111]),
        ('p', [0b110, 0b101, 0b110, 0b100, 0b100]),
        ('y', [0b101, 0b101, 0b010, 0b010, 0b010]),
        ('P', [0b110, 0b101, 0b110, 0b100, 0b100]),
        ('i', [0b010, 0b000, 0b010, 0b010, 0b010]),
        ('n', [0b000, 0b110, 0b101, 0b101, 0b101]),
        ('S', [0b111, 0b100, 0b111, 0b001, 0b111]),
        ('a', [0b000, 0b011, 0b101, 0b101, 0b011]),
        ('v', [0b000, 0b101, 0b101, 0b101, 0b010]),
        ('O', [0b111, 0b101, 0b101, 0b101, 0b111]),
        ('R', [0b110, 0b101, 0b110, 0b101, 0b101]),
        ('A', [0b010, 0b101, 0b111, 0b101, 0b101]),
        ('E', [0b111, 0b100, 0b111, 0b100, 0b111]),
        ('M', [0b101, 0b111, 0b111, 0b101, 0b101]),
        ('L', [0b100, 0b100, 0b100, 0b100, 0b111]),
        ('N', [0b101, 0b111, 0b111, 0b111, 0b101]),
        ('T', [0b111, 0b010, 0b010, 0b010, 0b010]),
    ];
    let chars: Vec<char> = mark.chars().take(3).collect();
    let total_w = chars.len() as i32 * 4 - 1;
    let mut cx = rect.origin.x + (rect.size.width as i32 - total_w) / 2;
    let cy = rect.origin.y + (rect.size.height as i32 - 5) / 2;
    for ch in chars {
        let bits = glyphs.iter().find(|(c, _)| *c == ch).map(|(_, b)| *b);
        if let Some(rows) = bits {
            for (row, mask) in rows.iter().enumerate() {
                for col in 0..3 {
                    if mask & (1 << (2 - col)) != 0 {
                        let x = cx + col;
                        let y = cy + row as i32;
                        if x >= 0 && y >= 0 && (x as usize) < stride && (y as usize) < height {
                            let i = y as usize * stride + x as usize;
                            if i < frame.len() {
                                frame[i] = color;
                            }
                        }
                    }
                }
            }
        }
        cx += 4;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_and_hit() {
        let sel = PixelRect::new(100, 100, 200, 100);
        let buttons = layout_toolbar(sel, 800, 600);
        assert!(!buttons.is_empty());
        let mid = buttons[0].rect;
        let p = PixelPoint::new(mid.origin.x + 2, mid.origin.y + 2);
        assert_eq!(hit_test(&buttons, p), Some(ToolbarAction::Copy));
    }

    #[test]
    fn tool_hit() {
        let sel = PixelRect::new(50, 50, 400, 80);
        let buttons = layout_toolbar(sel, 1000, 800);
        let rect_btn = buttons
            .iter()
            .find(|b| b.action == ToolbarAction::Tool(AnnotateTool::Rect))
            .unwrap();
        let p = PixelPoint::new(rect_btn.rect.origin.x + 1, rect_btn.rect.origin.y + 1);
        assert_eq!(
            hit_test(&buttons, p),
            Some(ToolbarAction::Tool(AnnotateTool::Rect))
        );
    }

    #[test]
    fn line_and_number_tools_are_available_and_hittable() {
        let buttons = layout_toolbar(PixelRect::new(50, 50, 400, 80), 1000, 800);
        for tool in [AnnotateTool::Line, AnnotateTool::Number] {
            let button = buttons
                .iter()
                .find(|button| button.action == ToolbarAction::Tool(tool))
                .expect("new tool button");
            let point = PixelPoint::new(button.rect.origin.x + 1, button.rect.origin.y + 1);
            assert_eq!(hit_test(&buttons, point), Some(ToolbarAction::Tool(tool)));
        }
    }

    #[test]
    fn narrow_canvas_wraps_buttons_without_leaving_the_canvas() {
        let buttons = layout_toolbar(PixelRect::new(20, 20, 200, 100), 300, 200);
        assert!(
            buttons
                .iter()
                .any(|button| { button.action == ToolbarAction::Tool(AnnotateTool::ColorPicker) })
        );
        assert!(
            buttons
                .iter()
                .any(|button| button.rect.origin.y > buttons[0].rect.origin.y)
        );
        for button in &buttons {
            assert!(button.rect.origin.x >= 0);
            assert!(button.rect.origin.y >= 0);
            assert!(button.rect.right() <= 300);
            assert!(button.rect.bottom() <= 200);
            let point = PixelPoint::new(button.rect.origin.x + 1, button.rect.origin.y + 1);
            assert_eq!(hit_test(&buttons, point), Some(button.action));
        }
    }

    #[test]
    fn toolbar_hides_instead_of_rendering_clipped_controls() {
        assert!(layout_toolbar(PixelRect::new(0, 0, 50, 50), 70, 200).is_empty());
        assert!(layout_toolbar(PixelRect::new(0, 0, 50, 50), 300, 100).is_empty());
    }
}
