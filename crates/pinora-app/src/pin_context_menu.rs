//! 贴图客户区内的上下文菜单。
//!
//! 菜单不创建原生窗口，避免临时操作 UI 进入任务栏或 Dock。调用方负责把动作映射到
//! 已有贴图的命令/任务，并在窗口重绘时调用 [`paint`].

use pinora_core::{PixelPoint, PixelRect};

use crate::settings_panel::draw_text;

const MENU_WIDTH: u32 = 152;
const ITEM_HEIGHT: u32 = 24;
const MENU_PADDING: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PinMenuAction {
    Copy,
    Ocr,
    Edit,
    FitToImage,
    ToggleLock,
    OpacityDown,
    OpacityUp,
    ToggleAlwaysOnTop,
    Save,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PinMenuItem {
    pub action: PinMenuAction,
    pub rect: PixelRect,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PinContextMenu {
    pub bounds: PixelRect,
    pub items: Vec<PinMenuItem>,
}

impl PinContextMenu {
    pub(crate) fn open(
        anchor: PixelPoint,
        window_width: u32,
        window_height: u32,
        locked: bool,
    ) -> Self {
        let specs = [
            (PinMenuAction::Copy, true),
            (PinMenuAction::Ocr, true),
            (PinMenuAction::Edit, !locked),
            (PinMenuAction::FitToImage, !locked),
            (PinMenuAction::ToggleLock, true),
            (PinMenuAction::OpacityDown, !locked),
            (PinMenuAction::OpacityUp, !locked),
            (PinMenuAction::ToggleAlwaysOnTop, true),
            (PinMenuAction::Save, true),
            (PinMenuAction::Close, true),
        ];
        let width = MENU_WIDTH.min(window_width.max(1));
        let desired_height = MENU_PADDING * 2 + ITEM_HEIGHT * specs.len() as u32;
        let height = desired_height.min(window_height.max(1));
        let max_x = i32::try_from(window_width.saturating_sub(width)).unwrap_or(i32::MAX);
        let max_y = i32::try_from(window_height.saturating_sub(height)).unwrap_or(i32::MAX);
        let x = anchor.x.clamp(0, max_x);
        let y = anchor.y.clamp(0, max_y);
        let bounds = PixelRect::new(x, y, width, height);

        let mut items = Vec::with_capacity(specs.len());
        let content_height = height.saturating_sub(MENU_PADDING * 2);
        // 菜单只能留在当前贴图客户区。对常见的小贴图压缩行高，优先保证全部动作
        // 可见而非截断列表；极端小于九物理像素的窗口无法提供可点击菜单。
        let item_height = (content_height / specs.len() as u32).clamp(1, ITEM_HEIGHT);
        let mut item_y = y + MENU_PADDING.min(height / 2) as i32;
        let item_width = width.saturating_sub(MENU_PADDING * 2);
        let item_bottom = bounds.bottom();
        for (action, enabled) in specs {
            if item_y >= item_bottom {
                break;
            }
            let remaining = (item_bottom - item_y).max(0) as u32;
            let row_height = item_height.min(remaining);
            if row_height == 0 {
                break;
            }
            items.push(PinMenuItem {
                action,
                rect: PixelRect::new(x + MENU_PADDING as i32, item_y, item_width, row_height),
                enabled,
            });
            item_y += item_height as i32;
        }

        Self { bounds, items }
    }

    pub(crate) fn hit_test(&self, point: PixelPoint) -> Option<PinMenuAction> {
        self.items
            .iter()
            .find(|item| item.enabled && item.rect.contains_point(point))
            .map(|item| item.action)
    }

    pub(crate) fn contains(&self, point: PixelPoint) -> bool {
        self.bounds.contains_point(point)
    }
}

pub(crate) fn paint(
    frame: &mut [u32],
    stride: usize,
    height: usize,
    menu: &PinContextMenu,
    locked: bool,
    always_on_top: bool,
) {
    fill_rect(frame, stride, height, menu.bounds, 0x00_1F_22_2A);
    outline_rect(frame, stride, height, menu.bounds, 0x00_93_A4_C7);
    for item in &menu.items {
        let color = if !item.enabled {
            0x00_3A_3D_45
        } else if item.action == PinMenuAction::Close {
            0x00_8A_36_36
        } else {
            0x00_30_3A_4E
        };
        fill_rect(frame, stride, height, item.rect, color);
        let label = label(item.action, locked, always_on_top);
        let label_width = label.len().saturating_mul(6) as i32;
        let label_x = item.rect.origin.x + (item.rect.size.width as i32 - label_width) / 2;
        let label_y = item.rect.origin.y + (item.rect.size.height as i32 - 5) / 2;
        draw_text(
            frame,
            stride,
            height,
            label_x,
            label_y,
            label,
            if item.enabled {
                0x00_F4_F7_FF
            } else {
                0x00_8C_93_A3
            },
        );
    }
}

fn label(action: PinMenuAction, locked: bool, always_on_top: bool) -> &'static str {
    match action {
        PinMenuAction::Copy => "COPY",
        PinMenuAction::Ocr => "OCR",
        PinMenuAction::Edit => "EDIT",
        PinMenuAction::FitToImage => "100%",
        PinMenuAction::ToggleLock if locked => "UNLOCK",
        PinMenuAction::ToggleLock => "LOCK",
        PinMenuAction::OpacityDown => "DIM -",
        PinMenuAction::OpacityUp => "DIM +",
        PinMenuAction::ToggleAlwaysOnTop if always_on_top => "TOP",
        PinMenuAction::ToggleAlwaysOnTop => "BASE",
        PinMenuAction::Save => "SAVE",
        PinMenuAction::Close => "CLOSE",
    }
}

fn fill_rect(frame: &mut [u32], stride: usize, height: usize, rect: PixelRect, color: u32) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = rect.right().max(0) as usize;
    let y1 = rect.bottom().max(0) as usize;
    for y in y0.min(height)..y1.min(height) {
        let start = y * stride + x0.min(stride);
        let end = y * stride + x1.min(stride);
        if start < end && end <= frame.len() {
            frame[start..end].fill(color);
        }
    }
}

fn outline_rect(frame: &mut [u32], stride: usize, height: usize, rect: PixelRect, color: u32) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = rect.right().saturating_sub(1).max(0) as usize;
    let y1 = rect.bottom().saturating_sub(1).max(0) as usize;
    if x0 > x1 || y0 > y1 || x0 >= stride || y0 >= height {
        return;
    }
    let x1 = x1.min(stride - 1);
    let y1 = y1.min(height - 1);
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
    fn menu_stays_within_the_current_pin_window() {
        let menu = PinContextMenu::open(PixelPoint::new(190, 115), 200, 120, false);

        assert!(menu.bounds.origin.x >= 0);
        assert!(menu.bounds.origin.y >= 0);
        assert!(menu.bounds.right() <= 200);
        assert!(menu.bounds.bottom() <= 120);
        assert_eq!(menu.items.len(), 10);
    }

    #[test]
    fn locked_menu_keeps_copy_ocr_lock_and_close_but_disables_mutating_items() {
        let menu = PinContextMenu::open(PixelPoint::new(0, 0), 320, 320, true);
        let enabled = |action| {
            menu.items
                .iter()
                .find(|item| item.action == action)
                .unwrap()
                .enabled
        };

        assert!(enabled(PinMenuAction::Copy));
        assert!(enabled(PinMenuAction::Ocr));
        assert!(enabled(PinMenuAction::ToggleLock));
        assert!(enabled(PinMenuAction::Close));
        assert!(!enabled(PinMenuAction::Edit));
        assert!(!enabled(PinMenuAction::FitToImage));
        assert!(!enabled(PinMenuAction::OpacityDown));
        assert!(!enabled(PinMenuAction::OpacityUp));
    }

    #[test]
    fn unlocked_menu_offers_fit_to_image() {
        let menu = PinContextMenu::open(PixelPoint::new(0, 0), 320, 320, false);

        assert!(
            menu.items
                .iter()
                .any(|item| { item.action == PinMenuAction::FitToImage && item.enabled })
        );
    }

    #[test]
    fn hit_test_only_returns_enabled_items() {
        let menu = PinContextMenu::open(PixelPoint::new(0, 0), 320, 320, true);
        let copy = menu.items[0];
        let edit = menu.items[2];

        assert_eq!(
            menu.hit_test(PixelPoint::new(copy.rect.origin.x, copy.rect.origin.y)),
            Some(PinMenuAction::Copy)
        );
        assert_eq!(
            menu.hit_test(PixelPoint::new(edit.rect.origin.x, edit.rect.origin.y)),
            None
        );
        assert!(menu.contains(PixelPoint::new(edit.rect.origin.x, edit.rect.origin.y)));
        assert!(!menu.contains(PixelPoint::new(-1, -1)));
    }
}
