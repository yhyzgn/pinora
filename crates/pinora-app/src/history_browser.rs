//! 历史浏览窗口的纯状态、布局与预览绘制。
//!
//! 文件校验、PNG 解码和 tombstone 删除在 `history_export` 中完成；本模块不接触
//! 文件系统，也不持有窗口句柄。

use pinora_core::{HistoryEntry, PixelPoint, PixelRect, PixelSize};

use crate::settings_panel::{draw_outline, draw_text, fill};

pub const PANEL_WIDTH: u32 = 820;
pub const PANEL_HEIGHT: u32 = 520;
const LIST_X: i32 = 24;
const LIST_Y: i32 = 68;
const LIST_W: u32 = 410;
const ROW_H: u32 = 56;
const ROW_GAP: i32 = 8;
const MAX_VISIBLE_ROWS: usize = 6;
const PREVIEW: PixelRect = PixelRect::new(466, 68, 330, 336);
const BUTTON_Y: i32 = 442;
const BUTTON_W: u32 = 144;
const BUTTON_H: u32 = 42;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryPanelAction {
    Select(usize),
    Reopen,
    Delete,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryPanelKey {
    Up,
    Down,
    Enter,
    Delete,
    Escape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryPanelStatus {
    Ready,
    Empty,
    Error(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPanel {
    entries: Vec<HistoryEntry>,
    selected: Option<usize>,
    first_visible: usize,
    status: HistoryPanelStatus,
}

pub struct HistoryPreview<'a> {
    pub pixels_xrgb: &'a [u32],
    pub size: PixelSize,
}

impl HistoryPanel {
    pub fn new(entries: Vec<HistoryEntry>) -> Self {
        let mut panel = Self {
            entries,
            selected: None,
            first_visible: 0,
            status: HistoryPanelStatus::Empty,
        };
        panel.reset_selection();
        panel
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn selected_entry(&self) -> Option<&HistoryEntry> {
        self.selected.and_then(|index| self.entries.get(index))
    }

    pub const fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub const fn status(&self) -> &HistoryPanelStatus {
        &self.status
    }

    pub fn replace_entries(&mut self, entries: Vec<HistoryEntry>) {
        self.entries = entries;
        self.reset_selection();
    }

    pub fn mark_error(&mut self, code: &'static str) {
        self.status = HistoryPanelStatus::Error(code);
    }

    pub fn clear_error(&mut self) {
        self.status = if self.entries.is_empty() {
            HistoryPanelStatus::Empty
        } else {
            HistoryPanelStatus::Ready
        };
    }

    pub fn select(&mut self, index: usize) {
        if index >= self.entries.len() {
            return;
        }
        self.selected = Some(index);
        self.status = HistoryPanelStatus::Ready;
        self.ensure_selected_visible();
    }

    pub fn handle_key(&mut self, key: HistoryPanelKey) -> Option<HistoryPanelAction> {
        match key {
            HistoryPanelKey::Up => {
                if let Some(selected) = self.selected {
                    self.select(selected.saturating_sub(1));
                }
                None
            }
            HistoryPanelKey::Down => {
                if let Some(selected) = self.selected {
                    self.select((selected + 1).min(self.entries.len().saturating_sub(1)));
                }
                None
            }
            HistoryPanelKey::Enter => self.selected.map(|_| HistoryPanelAction::Reopen),
            HistoryPanelKey::Delete => self.selected.map(|_| HistoryPanelAction::Delete),
            HistoryPanelKey::Escape => Some(HistoryPanelAction::Close),
        }
    }

    pub fn hit_test(&self, point: PixelPoint) -> Option<HistoryPanelAction> {
        for row in 0..self.visible_count() {
            let index = self.first_visible + row;
            if row_rect(row).contains_point(point) {
                return Some(HistoryPanelAction::Select(index));
            }
        }
        if reopen_rect().contains_point(point) && self.selected.is_some() {
            return Some(HistoryPanelAction::Reopen);
        }
        if delete_rect().contains_point(point) && self.selected.is_some() {
            return Some(HistoryPanelAction::Delete);
        }
        if close_rect().contains_point(point) {
            return Some(HistoryPanelAction::Close);
        }
        None
    }

    pub fn visible_count(&self) -> usize {
        self.entries
            .len()
            .saturating_sub(self.first_visible)
            .min(MAX_VISIBLE_ROWS)
    }

    pub fn visible_entry(&self, row: usize) -> Option<&HistoryEntry> {
        self.entries.get(self.first_visible + row)
    }

    fn reset_selection(&mut self) {
        self.first_visible = 0;
        self.selected = (!self.entries.is_empty()).then_some(0);
        self.status = if self.entries.is_empty() {
            HistoryPanelStatus::Empty
        } else {
            HistoryPanelStatus::Ready
        };
    }

    fn ensure_selected_visible(&mut self) {
        let Some(selected) = self.selected else {
            return;
        };
        if selected < self.first_visible {
            self.first_visible = selected;
        } else if selected >= self.first_visible + MAX_VISIBLE_ROWS {
            self.first_visible = selected + 1 - MAX_VISIBLE_ROWS;
        }
    }
}

pub const fn row_rect(row: usize) -> PixelRect {
    PixelRect::new(
        LIST_X,
        LIST_Y + row as i32 * (ROW_H as i32 + ROW_GAP),
        LIST_W,
        ROW_H,
    )
}

pub const fn reopen_rect() -> PixelRect {
    PixelRect::new(466, BUTTON_Y, BUTTON_W, BUTTON_H)
}

pub const fn delete_rect() -> PixelRect {
    PixelRect::new(626, BUTTON_Y, BUTTON_W, BUTTON_H)
}

pub const fn close_rect() -> PixelRect {
    PixelRect::new(24, BUTTON_Y, BUTTON_W, BUTTON_H)
}

pub fn paint(
    panel: &HistoryPanel,
    preview: Option<HistoryPreview<'_>>,
    frame: &mut [u32],
    stride: usize,
    height: usize,
) {
    fill(
        frame,
        stride,
        height,
        PixelRect::new(0, 0, PANEL_WIDTH, PANEL_HEIGHT),
        0x00141820,
    );
    fill(
        frame,
        stride,
        height,
        PixelRect::new(0, 0, PANEL_WIDTH, 48),
        0x00202B3A,
    );
    draw_outline(
        frame,
        stride,
        height,
        PixelRect::new(0, 0, PANEL_WIDTH, PANEL_HEIGHT),
        0x005A718A,
    );
    let title = match panel.status() {
        HistoryPanelStatus::Ready => "HISTORY  UP DOWN SELECT  ENTER PIN  DELETE REMOVE",
        HistoryPanelStatus::Empty => "HISTORY EMPTY  ESC CLOSE",
        HistoryPanelStatus::Error(_) => "HISTORY FILE INVALID  DELETE OR SELECT ANOTHER",
    };
    draw_text(frame, stride, height, 24, 22, title, 0x00D8E6F3);

    if panel.entries().is_empty() {
        draw_outline(frame, stride, height, PREVIEW, 0x004B637A);
        draw_text(frame, stride, height, 330, 220, "NO ITEMS", 0x00A0B2C4);
    }
    for row in 0..panel.visible_count() {
        let index = panel.first_visible + row;
        let Some(entry) = panel.visible_entry(row) else {
            continue;
        };
        let rect = row_rect(row);
        let selected = panel.selected_index() == Some(index);
        fill(
            frame,
            stride,
            height,
            rect,
            if selected { 0x002B5270 } else { 0x00202A36 },
        );
        draw_outline(frame, stride, height, rect, 0x004B637A);
        draw_text(
            frame,
            stride,
            height,
            rect.origin.x + 12,
            rect.origin.y + 14,
            &format!("IMG {}", entry.image_id.raw()),
            0x00E5EDF5,
        );
        draw_text(
            frame,
            stride,
            height,
            rect.origin.x + 12,
            rect.origin.y + 32,
            &format!(
                "{} X {}",
                entry.source_rect.size.width, entry.source_rect.size.height
            ),
            0x00B5D8FF,
        );
    }

    fill(frame, stride, height, PREVIEW, 0x001D2630);
    draw_outline(frame, stride, height, PREVIEW, 0x004B637A);
    if let Some(preview) = preview {
        draw_preview(frame, stride, height, preview);
    } else if panel.selected_entry().is_some() {
        draw_text(
            frame,
            stride,
            height,
            PREVIEW.origin.x + 82,
            PREVIEW.origin.y + 168,
            "NO PREVIEW",
            0x00A0B2C4,
        );
    }

    fill(frame, stride, height, reopen_rect(), 0x002C6EA3);
    fill(frame, stride, height, delete_rect(), 0x007A3B3B);
    fill(frame, stride, height, close_rect(), 0x00434D59);
    draw_outline(frame, stride, height, reopen_rect(), 0x0096C8FF);
    draw_outline(frame, stride, height, delete_rect(), 0x00E39A9A);
    draw_outline(frame, stride, height, close_rect(), 0x007A8998);
    draw_text(
        frame,
        stride,
        height,
        reopen_rect().origin.x + 28,
        reopen_rect().origin.y + 17,
        "PIN",
        0x00FFFFFF,
    );
    draw_text(
        frame,
        stride,
        height,
        delete_rect().origin.x + 35,
        delete_rect().origin.y + 17,
        "DELETE",
        0x00FFFFFF,
    );
    draw_text(
        frame,
        stride,
        height,
        close_rect().origin.x + 32,
        close_rect().origin.y + 17,
        "CLOSE",
        0x00FFFFFF,
    );
}

fn draw_preview(frame: &mut [u32], stride: usize, height: usize, preview: HistoryPreview<'_>) {
    let src_w = preview.size.width as usize;
    let src_h = preview.size.height as usize;
    if src_w == 0 || src_h == 0 || preview.pixels_xrgb.len() < src_w.saturating_mul(src_h) {
        return;
    }
    let max_w = PREVIEW.size.width.saturating_sub(16) as usize;
    let max_h = PREVIEW.size.height.saturating_sub(16) as usize;
    let scale = (max_w as f64 / src_w as f64)
        .min(max_h as f64 / src_h as f64)
        .min(1.0);
    let dst_w = ((src_w as f64 * scale).round() as usize).max(1);
    let dst_h = ((src_h as f64 * scale).round() as usize).max(1);
    let x0 = PREVIEW.origin.x + (PREVIEW.size.width as i32 - dst_w as i32) / 2;
    let y0 = PREVIEW.origin.y + (PREVIEW.size.height as i32 - dst_h as i32) / 2;
    for y in 0..dst_h {
        let dy = y0 + y as i32;
        if dy < 0 || dy as usize >= height {
            continue;
        }
        let sy = y * src_h / dst_h;
        for x in 0..dst_w {
            let dx = x0 + x as i32;
            if dx < 0 || dx as usize >= stride {
                continue;
            }
            let sx = x * src_w / dst_w;
            frame[dy as usize * stride + dx as usize] = preview.pixels_xrgb[sy * src_w + sx];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{
        AssetGeneration, ContentDigest, DisplayId, HistoryEntrySpec, HistoryOcrState, ImageId,
    };

    fn entry(id: u64) -> HistoryEntry {
        HistoryEntry::new(HistoryEntrySpec {
            image_id: ImageId::from_raw(id),
            generation: AssetGeneration::INITIAL,
            created_at_ms: id,
            display: DisplayId::new("display-0"),
            source_rect: PixelRect::new(0, 0, 20, 10),
            file_name: format!("{id}.png"),
            byte_len: 1,
            digest: ContentDigest::of(&[id as u8]),
            ocr: HistoryOcrState::Unknown,
        })
        .expect("entry")
    }

    #[test]
    fn selection_scrolls_and_keyboard_actions_require_an_entry() {
        let mut panel = HistoryPanel::new((1..=8).map(entry).collect());
        for _ in 0..6 {
            assert_eq!(panel.handle_key(HistoryPanelKey::Down), None);
        }
        assert_eq!(panel.selected_index(), Some(6));
        assert_eq!(
            panel.visible_entry(0).map(|entry| entry.image_id.raw()),
            Some(2)
        );
        assert_eq!(
            panel.handle_key(HistoryPanelKey::Enter),
            Some(HistoryPanelAction::Reopen)
        );
        assert_eq!(
            panel.handle_key(HistoryPanelKey::Delete),
            Some(HistoryPanelAction::Delete)
        );
        let empty = HistoryPanel::new(Vec::new());
        assert_eq!(empty.selected_entry(), None);
    }

    #[test]
    fn hit_test_maps_rows_and_commands() {
        let panel = HistoryPanel::new(vec![entry(1)]);
        assert_eq!(
            panel.hit_test(PixelPoint::new(40, LIST_Y + 8)),
            Some(HistoryPanelAction::Select(0))
        );
        assert_eq!(
            panel.hit_test(PixelPoint::new(reopen_rect().origin.x + 4, BUTTON_Y + 4)),
            Some(HistoryPanelAction::Reopen)
        );
        assert_eq!(
            panel.hit_test(PixelPoint::new(delete_rect().origin.x + 4, BUTTON_Y + 4)),
            Some(HistoryPanelAction::Delete)
        );
    }
}
