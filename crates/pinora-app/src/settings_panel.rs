//! 设置窗口的纯状态与自绘布局。
//!
//! 该模块不依赖 winit 或文件系统。桌面壳只负责把窗口事件转换为
//! [`SettingsPanelAction`]，并在确认保存后调用 `SettingsStore`。

use pinora_core::{
    AppSettings, ExportImageFormat, HISTORY_MAX_BYTES_MAX, HISTORY_MAX_BYTES_MIN,
    HISTORY_RETENTION_DAYS_MAX, HISTORY_RETENTION_DAYS_MIN, HotkeyBinding, OcrLanguage, PixelPoint,
    PixelRect, REGION_ALTERNATE_HOTKEY, REGION_SECONDARY_HOTKEY, ThemeMode,
};

use crate::panel_theme::PanelTheme;

pub const PANEL_WIDTH: u32 = 560;
pub const PANEL_HEIGHT: u32 = 950;

const ROW_X: i32 = 28;
const ROW_W: u32 = PANEL_WIDTH - 56;
const ROW_H: u32 = 54;
const ROW_GAP: i32 = 8;
const FIRST_ROW_Y: i32 = 74;
const BUTTON_W: u32 = 112;
const BUTTON_H: u32 = 42;
const BUTTON_Y: i32 = PANEL_HEIGHT as i32 - 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingField {
    Theme,
    HistoryLimit,
    HistoryRetentionDays,
    HistoryMaxBytes,
    PinLimit,
    PinOpacity,
    DefaultPinAlwaysOnTop,
    OcrLanguage,
    OcrConfidenceThreshold,
    ExportFormat,
    JpegQuality,
    RegionHotkey,
    FullDisplayHotkey,
}

impl SettingField {
    pub const ALL: [Self; 13] = [
        Self::Theme,
        Self::HistoryLimit,
        Self::HistoryRetentionDays,
        Self::HistoryMaxBytes,
        Self::PinLimit,
        Self::PinOpacity,
        Self::DefaultPinAlwaysOnTop,
        Self::OcrLanguage,
        Self::OcrConfidenceThreshold,
        Self::ExportFormat,
        Self::JpegQuality,
        Self::RegionHotkey,
        Self::FullDisplayHotkey,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::Theme => 0,
            Self::HistoryLimit => 1,
            Self::HistoryRetentionDays => 2,
            Self::HistoryMaxBytes => 3,
            Self::PinLimit => 4,
            Self::PinOpacity => 5,
            Self::DefaultPinAlwaysOnTop => 6,
            Self::OcrLanguage => 7,
            Self::OcrConfidenceThreshold => 8,
            Self::ExportFormat => 9,
            Self::JpegQuality => 10,
            Self::RegionHotkey => 11,
            Self::FullDisplayHotkey => 12,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Theme => "THEME",
            Self::HistoryLimit => "HISTORY LIMIT",
            Self::HistoryRetentionDays => "HISTORY RETENTION",
            Self::HistoryMaxBytes => "HISTORY DISK LIMIT",
            Self::PinLimit => "PIN LIMIT",
            Self::PinOpacity => "PIN OPACITY",
            Self::DefaultPinAlwaysOnTop => "PIN ALWAYS ON TOP",
            Self::OcrLanguage => "OCR LANGUAGE",
            Self::OcrConfidenceThreshold => "OCR CONFIDENCE",
            Self::ExportFormat => "EXPORT FORMAT",
            Self::JpegQuality => "JPEG QUALITY",
            Self::RegionHotkey => "REGION HOTKEY",
            Self::FullDisplayHotkey => "FULL DISPLAY HOTKEY",
        }
    }

    pub const fn row_rect(self) -> PixelRect {
        PixelRect::new(
            ROW_X,
            FIRST_ROW_Y + self.index() as i32 * (ROW_H as i32 + ROW_GAP),
            ROW_W,
            ROW_H,
        )
    }

    pub const fn is_hotkey(self) -> bool {
        matches!(self, Self::RegionHotkey | Self::FullDisplayHotkey)
    }

    pub const fn is_binary_toggle(self) -> bool {
        matches!(self, Self::DefaultPinAlwaysOnTop)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPanelAction {
    Select(SettingField),
    Decrement,
    Increment,
    StartHotkeyRecording,
    Save,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPanelKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsPanelStatus {
    Editing,
    Recording(SettingField),
    Saved,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsPanel {
    original: AppSettings,
    draft: AppSettings,
    selected: SettingField,
    recording: Option<SettingField>,
    status: SettingsPanelStatus,
}

impl SettingsPanel {
    pub fn new(settings: AppSettings) -> Self {
        let (settings, _) = settings.with_repaired_values();
        Self {
            original: settings,
            draft: settings,
            selected: SettingField::Theme,
            recording: None,
            status: SettingsPanelStatus::Editing,
        }
    }

    pub const fn draft(&self) -> AppSettings {
        self.draft
    }

    pub const fn original(&self) -> AppSettings {
        self.original
    }

    pub const fn selected(&self) -> SettingField {
        self.selected
    }

    pub const fn recording_field(&self) -> Option<SettingField> {
        self.recording
    }

    pub const fn status(&self) -> &SettingsPanelStatus {
        &self.status
    }

    pub fn is_dirty(&self) -> bool {
        self.draft != self.original
    }

    pub fn select(&mut self, field: SettingField) {
        self.selected = field;
        self.recording = None;
        if !matches!(self.status, SettingsPanelStatus::Editing) {
            self.status = SettingsPanelStatus::Editing;
        }
    }

    pub fn handle_key(&mut self, key: SettingsPanelKey) -> Option<SettingsPanelAction> {
        if self.recording.is_some() {
            if key == SettingsPanelKey::Escape {
                self.cancel_hotkey_recording();
            }
            return None;
        }
        match key {
            SettingsPanelKey::Up => {
                let index = self.selected.index();
                let next = if index == 0 {
                    SettingField::ALL.len() - 1
                } else {
                    index - 1
                };
                self.select(SettingField::ALL[next]);
                None
            }
            SettingsPanelKey::Down => {
                let next = (self.selected.index() + 1) % SettingField::ALL.len();
                self.select(SettingField::ALL[next]);
                None
            }
            SettingsPanelKey::Left => {
                self.step(-1);
                None
            }
            SettingsPanelKey::Right => {
                self.step(1);
                None
            }
            SettingsPanelKey::Enter if self.selected.is_hotkey() => {
                Some(SettingsPanelAction::StartHotkeyRecording)
            }
            SettingsPanelKey::Enter => Some(SettingsPanelAction::Save),
            SettingsPanelKey::Escape => Some(SettingsPanelAction::Cancel),
        }
    }

    pub fn apply_action(&mut self, action: SettingsPanelAction) {
        match action {
            SettingsPanelAction::Select(field) => self.select(field),
            SettingsPanelAction::Decrement => self.step(-1),
            SettingsPanelAction::Increment => self.step(1),
            SettingsPanelAction::StartHotkeyRecording
            | SettingsPanelAction::Save
            | SettingsPanelAction::Cancel => {}
        }
    }

    pub fn step(&mut self, direction: i32) {
        if direction == 0 {
            return;
        }
        match self.selected {
            SettingField::Theme => {
                const THEMES: [ThemeMode; 3] =
                    [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];
                let current = THEMES
                    .iter()
                    .position(|theme| *theme == self.draft.theme)
                    .unwrap_or(0) as i32;
                let next = (current + direction).rem_euclid(THEMES.len() as i32) as usize;
                self.draft.theme = THEMES[next];
            }
            SettingField::HistoryLimit => {
                self.draft.history_limit =
                    step_u32(self.draft.history_limit, direction, 1, 10_000, 10);
            }
            SettingField::HistoryRetentionDays => {
                self.draft.history_retention_days = step_u16(
                    self.draft.history_retention_days,
                    direction,
                    HISTORY_RETENTION_DAYS_MIN,
                    HISTORY_RETENTION_DAYS_MAX,
                    1,
                );
            }
            SettingField::HistoryMaxBytes => {
                self.draft.history_max_bytes = step_u64(
                    self.draft.history_max_bytes,
                    direction,
                    HISTORY_MAX_BYTES_MIN,
                    HISTORY_MAX_BYTES_MAX,
                    64 * 1024 * 1024,
                );
            }
            SettingField::PinLimit => {
                self.draft.pin_limit = step_u16(self.draft.pin_limit, direction, 1, 100, 1);
            }
            SettingField::PinOpacity => {
                self.draft.default_pin_opacity_percent = step_u8(
                    self.draft.default_pin_opacity_percent,
                    direction,
                    15,
                    100,
                    5,
                );
            }
            SettingField::DefaultPinAlwaysOnTop => {
                self.draft.default_pin_always_on_top = direction > 0;
            }
            SettingField::OcrLanguage => {
                const LANGUAGES: [OcrLanguage; 3] = [
                    OcrLanguage::Auto,
                    OcrLanguage::English,
                    OcrLanguage::SimplifiedChinese,
                ];
                let current = LANGUAGES
                    .iter()
                    .position(|language| *language == self.draft.ocr_language)
                    .unwrap_or(0) as i32;
                let next = (current + direction).rem_euclid(LANGUAGES.len() as i32) as usize;
                self.draft.ocr_language = LANGUAGES[next];
            }
            SettingField::OcrConfidenceThreshold => {
                self.draft.ocr_confidence_threshold =
                    step_u8(self.draft.ocr_confidence_threshold, direction, 0, 100, 5);
            }
            SettingField::ExportFormat => {
                const FORMATS: [ExportImageFormat; 3] = [
                    ExportImageFormat::Png,
                    ExportImageFormat::Jpeg,
                    ExportImageFormat::WebP,
                ];
                let current = FORMATS
                    .iter()
                    .position(|format| *format == self.draft.export_format)
                    .unwrap_or(0) as i32;
                let next = (current + direction).rem_euclid(FORMATS.len() as i32) as usize;
                self.draft.export_format = FORMATS[next];
            }
            SettingField::JpegQuality => {
                self.draft.jpeg_quality = step_u8(self.draft.jpeg_quality, direction, 1, 100, 5);
            }
            SettingField::RegionHotkey | SettingField::FullDisplayHotkey => {}
        }
        self.status = SettingsPanelStatus::Editing;
    }

    pub fn start_hotkey_recording(&mut self) {
        if !self.selected.is_hotkey() {
            return;
        }
        self.recording = Some(self.selected);
        self.status = SettingsPanelStatus::Recording(self.selected);
    }

    pub fn record_hotkey(&mut self, binding: HotkeyBinding) -> Result<(), &'static str> {
        let Some(field) = self.recording else {
            return Err("hotkey_not_recording");
        };
        if !binding.is_safe() {
            self.status = SettingsPanelStatus::Error("hotkey_unsafe".into());
            return Err("hotkey_unsafe");
        }
        let conflict = match field {
            SettingField::RegionHotkey => {
                binding == self.draft.full_display_hotkey
                    || binding == REGION_SECONDARY_HOTKEY
                    || binding == REGION_ALTERNATE_HOTKEY
            }
            SettingField::FullDisplayHotkey => {
                binding == self.draft.region_hotkey
                    || binding == REGION_SECONDARY_HOTKEY
                    || binding == REGION_ALTERNATE_HOTKEY
            }
            _ => true,
        };
        if conflict {
            self.status = SettingsPanelStatus::Error("hotkey_conflict".into());
            return Err("hotkey_conflict");
        }
        match field {
            SettingField::RegionHotkey => self.draft.region_hotkey = binding,
            SettingField::FullDisplayHotkey => self.draft.full_display_hotkey = binding,
            _ => return Err("hotkey_not_recording"),
        }
        self.recording = None;
        self.status = SettingsPanelStatus::Editing;
        Ok(())
    }

    pub fn reject_hotkey_recording(&mut self, code: &'static str) {
        if self.recording.is_some() {
            self.status = SettingsPanelStatus::Error(code.into());
        }
    }

    pub fn cancel_hotkey_recording(&mut self) {
        self.recording = None;
        self.status = SettingsPanelStatus::Editing;
    }

    pub fn mark_saved(&mut self) {
        self.original = self.draft;
        self.recording = None;
        self.status = SettingsPanelStatus::Saved;
    }

    pub fn mark_save_failed(&mut self, error: impl Into<String>) {
        self.status = SettingsPanelStatus::Error(sanitize_error(error.into()));
    }

    pub fn cancel(&mut self) {
        self.draft = self.original;
        self.recording = None;
        self.status = SettingsPanelStatus::Editing;
    }

    pub fn hit_test(point: PixelPoint) -> Option<SettingsPanelAction> {
        for field in SettingField::ALL {
            if field.row_rect().contains_point(point) {
                let row = field.row_rect();
                if field.is_binary_toggle() && toggle_off_rect(row).contains_point(point) {
                    return Some(SettingsPanelAction::Decrement);
                }
                if field.is_binary_toggle() && toggle_on_rect(row).contains_point(point) {
                    return Some(SettingsPanelAction::Increment);
                }
                let minus = PixelRect::new(row.right() - 82, row.origin.y + 9, 30, 36);
                let plus = PixelRect::new(row.right() - 42, row.origin.y + 9, 30, 36);
                if !field.is_hotkey() && !field.is_binary_toggle() && minus.contains_point(point) {
                    return Some(SettingsPanelAction::Decrement);
                }
                if !field.is_hotkey() && !field.is_binary_toggle() && plus.contains_point(point) {
                    return Some(SettingsPanelAction::Increment);
                }
                return Some(SettingsPanelAction::Select(field));
            }
        }
        let save = save_rect();
        if save.contains_point(point) {
            return Some(SettingsPanelAction::Save);
        }
        if cancel_rect().contains_point(point) {
            return Some(SettingsPanelAction::Cancel);
        }
        None
    }

    pub fn value_label(&self, field: SettingField) -> String {
        match field {
            SettingField::Theme => match self.draft.theme {
                ThemeMode::System => "SYSTEM".into(),
                ThemeMode::Light => "LIGHT".into(),
                ThemeMode::Dark => "DARK".into(),
            },
            SettingField::HistoryLimit => format!("{} ITEMS", self.draft.history_limit),
            SettingField::HistoryRetentionDays => {
                format!("{} DAYS", self.draft.history_retention_days)
            }
            SettingField::HistoryMaxBytes => format_history_capacity(self.draft.history_max_bytes),
            SettingField::PinLimit => format!("{} PINS", self.draft.pin_limit),
            SettingField::PinOpacity => format!("{}%", self.draft.default_pin_opacity_percent),
            SettingField::DefaultPinAlwaysOnTop => {
                if self.draft.default_pin_always_on_top {
                    "ON".into()
                } else {
                    "OFF".into()
                }
            }
            SettingField::OcrLanguage => match self.draft.ocr_language {
                OcrLanguage::Auto => "AUTO".into(),
                OcrLanguage::English => "ENGLISH".into(),
                OcrLanguage::SimplifiedChinese => "SIMPLIFIED CHINESE".into(),
            },
            SettingField::OcrConfidenceThreshold => {
                format!("{}%", self.draft.ocr_confidence_threshold)
            }
            SettingField::ExportFormat => match self.draft.export_format {
                ExportImageFormat::Png => "PNG".into(),
                ExportImageFormat::Jpeg => "JPEG".into(),
                ExportImageFormat::WebP => "LOSSLESS WEBP".into(),
            },
            SettingField::JpegQuality => format!("{}%", self.draft.jpeg_quality),
            SettingField::RegionHotkey => self.draft.region_hotkey.to_string(),
            SettingField::FullDisplayHotkey => self.draft.full_display_hotkey.to_string(),
        }
    }
}

fn toggle_off_rect(row: PixelRect) -> PixelRect {
    PixelRect::new(row.right() - 112, row.origin.y + 9, 50, 36)
}

fn toggle_on_rect(row: PixelRect) -> PixelRect {
    PixelRect::new(row.right() - 52, row.origin.y + 9, 40, 36)
}

pub const fn save_rect() -> PixelRect {
    PixelRect::new(
        PANEL_WIDTH as i32 - 2 * BUTTON_W as i32 - 44,
        BUTTON_Y,
        BUTTON_W,
        BUTTON_H,
    )
}

pub const fn cancel_rect() -> PixelRect {
    PixelRect::new(
        PANEL_WIDTH as i32 - BUTTON_W as i32 - 28,
        BUTTON_Y,
        BUTTON_W,
        BUTTON_H,
    )
}

fn step_u32(value: u32, direction: i32, min: u32, max: u32, step: u32) -> u32 {
    if direction > 0 {
        value.saturating_add(step).min(max)
    } else {
        value.saturating_sub(step).max(min)
    }
}

fn step_u16(value: u16, direction: i32, min: u16, max: u16, step: u16) -> u16 {
    if direction > 0 {
        value.saturating_add(step).min(max)
    } else {
        value.saturating_sub(step).max(min)
    }
}

fn step_u8(value: u8, direction: i32, min: u8, max: u8, step: u8) -> u8 {
    if direction > 0 {
        value.saturating_add(step).min(max)
    } else {
        value.saturating_sub(step).max(min)
    }
}

fn step_u64(value: u64, direction: i32, min: u64, max: u64, step: u64) -> u64 {
    if direction > 0 {
        value.saturating_add(step).min(max)
    } else {
        value.saturating_sub(step).max(min)
    }
}

fn format_history_capacity(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * MIB;
    if bytes >= GIB && bytes.is_multiple_of(GIB) {
        format!("{} GB", bytes / GIB)
    } else {
        format!("{} MB", bytes / MIB)
    }
}

fn sanitize_error(error: String) -> String {
    let code = error
        .split_once(':')
        .map(|(code, _)| code)
        .unwrap_or(error.as_str());
    code.chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
        .take(64)
        .collect()
}

/// 将设置面板绘制到 XRGB 缓冲。文字使用短 ASCII 标记，避免依赖平台字体。
pub fn paint(
    panel: &SettingsPanel,
    theme: PanelTheme,
    frame: &mut [u32],
    stride: usize,
    height: usize,
) {
    fill(
        frame,
        stride,
        height,
        PixelRect::new(0, 0, PANEL_WIDTH, PANEL_HEIGHT),
        theme.background,
    );
    fill(
        frame,
        stride,
        height,
        PixelRect::new(0, 0, PANEL_WIDTH, 48),
        theme.header,
    );
    draw_outline(
        frame,
        stride,
        height,
        PixelRect::new(0, 0, PANEL_WIDTH, PANEL_HEIGHT),
        theme.border,
    );
    for field in SettingField::ALL {
        let row = field.row_rect();
        let bg = if panel.selected() == field {
            theme.selected_surface
        } else {
            theme.surface
        };
        fill(frame, stride, height, row, bg);
        draw_outline(frame, stride, height, row, theme.control_border);
        if field.is_binary_toggle() {
            let off = toggle_off_rect(row);
            let on = toggle_on_rect(row);
            let (off_color, off_border, off_text, on_color, on_border, on_text) =
                if panel.draft().default_pin_always_on_top {
                    (
                        theme.control_surface,
                        theme.control_border,
                        theme.primary_text,
                        theme.primary_action,
                        theme.primary_action_border,
                        theme.on_action_text,
                    )
                } else {
                    (
                        theme.primary_action,
                        theme.primary_action_border,
                        theme.on_action_text,
                        theme.control_surface,
                        theme.control_border,
                        theme.primary_text,
                    )
                };
            fill(frame, stride, height, off, off_color);
            fill(frame, stride, height, on, on_color);
            draw_outline(frame, stride, height, off, off_border);
            draw_outline(frame, stride, height, on, on_border);
            draw_text(
                frame,
                stride,
                height,
                off.origin.x + 15,
                off.origin.y + 15,
                "OFF",
                off_text,
            );
            draw_text(
                frame,
                stride,
                height,
                on.origin.x + 11,
                on.origin.y + 15,
                "ON",
                on_text,
            );
        } else if !field.is_hotkey() {
            let minus = PixelRect::new(row.right() - 82, row.origin.y + 9, 30, 36);
            let plus = PixelRect::new(row.right() - 42, row.origin.y + 9, 30, 36);
            fill(frame, stride, height, minus, theme.control_surface);
            fill(frame, stride, height, plus, theme.control_surface);
            draw_outline(frame, stride, height, minus, theme.control_border);
            draw_outline(frame, stride, height, plus, theme.control_border);
        }
        draw_text(
            frame,
            stride,
            height,
            row.origin.x + 14,
            row.origin.y + 16,
            field.label(),
            theme.primary_text,
        );
        draw_text(
            frame,
            stride,
            height,
            row.origin.x + 190,
            row.origin.y + 16,
            &panel.value_label(field),
            theme.secondary_text,
        );
        if !field.is_hotkey() && !field.is_binary_toggle() {
            let minus = PixelRect::new(row.right() - 82, row.origin.y + 9, 30, 36);
            let plus = PixelRect::new(row.right() - 42, row.origin.y + 9, 30, 36);
            draw_text(
                frame,
                stride,
                height,
                minus.origin.x + 11,
                minus.origin.y + 15,
                "-",
                theme.on_action_text,
            );
            draw_text(
                frame,
                stride,
                height,
                plus.origin.x + 11,
                plus.origin.y + 15,
                "+",
                theme.on_action_text,
            );
        }
    }
    let save_color = if matches!(panel.status(), SettingsPanelStatus::Saved) {
        theme.success_action
    } else {
        theme.primary_action
    };
    fill(frame, stride, height, save_rect(), save_color);
    fill(frame, stride, height, cancel_rect(), theme.secondary_action);
    draw_outline(
        frame,
        stride,
        height,
        save_rect(),
        if matches!(panel.status(), SettingsPanelStatus::Saved) {
            theme.success_action_border
        } else {
            theme.primary_action_border
        },
    );
    draw_outline(
        frame,
        stride,
        height,
        cancel_rect(),
        theme.secondary_action_border,
    );
    draw_text(
        frame,
        stride,
        height,
        save_rect().origin.x + 30,
        save_rect().origin.y + 17,
        "SAVE",
        theme.on_action_text,
    );
    draw_text(
        frame,
        stride,
        height,
        cancel_rect().origin.x + 23,
        cancel_rect().origin.y + 17,
        "CANCEL",
        theme.on_action_text,
    );
    let status = match panel.status() {
        SettingsPanelStatus::Editing if panel.selected().is_hotkey() => "ENTER RECORD  ESC CANCEL",
        SettingsPanelStatus::Editing => "ARROWS EDIT  ENTER SAVE  ESC CANCEL",
        SettingsPanelStatus::Recording(_) => "PRESS A HOTKEY  ESC CANCEL",
        SettingsPanelStatus::Saved => "SAVED",
        SettingsPanelStatus::Error(code) if code.starts_with("hotkey_") => {
            "HOTKEY REJECTED - TRY AGAIN"
        }
        SettingsPanelStatus::Error(_) => "SAVE FAILED - RETRY OR CANCEL",
    };
    draw_text(frame, stride, height, 28, 24, status, theme.primary_text);
}

/// 共享的自绘 XRGB 填充原语；仅用于应用内小型工具窗口。
pub(crate) fn fill(frame: &mut [u32], stride: usize, height: usize, rect: PixelRect, color: u32) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right().max(0) as usize).min(stride);
    let y1 = (rect.bottom().max(0) as usize).min(height);
    for y in y0..y1 {
        let start = y.saturating_mul(stride).saturating_add(x0);
        let end = y.saturating_mul(stride).saturating_add(x1).min(frame.len());
        if start < end {
            frame[start..end].fill(color);
        }
    }
}

/// 共享的自绘 XRGB 描边原语；仅用于应用内小型工具窗口。
pub(crate) fn draw_outline(
    frame: &mut [u32],
    stride: usize,
    height: usize,
    rect: PixelRect,
    color: u32,
) {
    if rect.size.width == 0 || rect.size.height == 0 {
        return;
    }
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = rect.right().saturating_sub(1).max(0) as usize;
    let y1 = rect.bottom().saturating_sub(1).max(0) as usize;
    for x in x0..=x1.min(stride.saturating_sub(1)) {
        if y0 < height {
            frame[y0 * stride + x] = color;
        }
        if y1 < height {
            frame[y1 * stride + x] = color;
        }
    }
    for y in y0..=y1.min(height.saturating_sub(1)) {
        if x0 < stride {
            frame[y * stride + x0] = color;
        }
        if x1 < stride {
            frame[y * stride + x1] = color;
        }
    }
}

/// 共享的简易 ASCII 点阵文字；不用于需要平台字体或无障碍语义的主界面。
pub(crate) fn draw_text(
    frame: &mut [u32],
    stride: usize,
    height: usize,
    x: i32,
    y: i32,
    text: &str,
    color: u32,
) {
    let mut cursor = x;
    for ch in text.chars() {
        draw_glyph(frame, stride, height, cursor, y, ch, color);
        cursor += 6;
    }
}

fn draw_glyph(
    frame: &mut [u32],
    stride: usize,
    height: usize,
    x: i32,
    y: i32,
    ch: char,
    color: u32,
) {
    let rows = match ch.to_ascii_uppercase() {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b111, 0b100, 0b100, 0b100, 0b111],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b111, 0b100, 0b101, 0b101, 0b111],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b111, 0b100, 0b111, 0b001, 0b111],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b110],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '_' => [0b000, 0b000, 0b000, 0b000, 0b111],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        ' ' => [0; 5],
        _ => [0b000, 0b010, 0b000, 0b010, 0b000],
    };
    for (row, bits) in rows.into_iter().enumerate() {
        for col in 0..3 {
            if bits & (1 << (2 - col)) == 0 {
                continue;
            }
            let px = x + col;
            let py = y + row as i32;
            if px >= 0 && py >= 0 && (px as usize) < stride && (py as usize) < height {
                frame[py as usize * stride + px as usize] = color;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel_theme::{PanelTheme, SystemAppearance};
    use pinora_core::{HotkeyCode, HotkeyModifiers};

    #[test]
    fn keyboard_navigation_wraps_and_steps_with_bounds() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        panel.handle_key(SettingsPanelKey::Up);
        assert_eq!(panel.selected(), SettingField::FullDisplayHotkey);
        panel.select(SettingField::OcrLanguage);
        panel.handle_key(SettingsPanelKey::Right);
        assert_eq!(panel.draft().ocr_language, OcrLanguage::English);
        panel.handle_key(SettingsPanelKey::Left);
        assert_eq!(panel.draft().ocr_language, OcrLanguage::Auto);
    }

    #[test]
    fn theme_cycles_without_invalid_wire_values() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        panel.step(-1);
        assert_eq!(panel.draft().theme, ThemeMode::Dark);
        panel.step(1);
        assert_eq!(panel.draft().theme, ThemeMode::System);
    }

    #[test]
    fn theme_draft_immediately_changes_the_settings_frame() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        let mut system_frame = vec![0; PANEL_WIDTH as usize * PANEL_HEIGHT as usize];
        paint(
            &panel,
            PanelTheme::resolve(panel.draft().theme, SystemAppearance::Unknown),
            &mut system_frame,
            PANEL_WIDTH as usize,
            PANEL_HEIGHT as usize,
        );

        panel.step(1);
        assert_eq!(panel.draft().theme, ThemeMode::Light);
        let mut light_frame = vec![0; PANEL_WIDTH as usize * PANEL_HEIGHT as usize];
        paint(
            &panel,
            PanelTheme::resolve(panel.draft().theme, SystemAppearance::Unknown),
            &mut light_frame,
            PANEL_WIDTH as usize,
            PANEL_HEIGHT as usize,
        );

        let background_pixel = 60 * PANEL_WIDTH as usize + 10;
        assert_eq!(
            system_frame[background_pixel],
            PanelTheme::dark().background
        );
        assert_eq!(
            light_frame[background_pixel],
            PanelTheme::light().background
        );
        assert_ne!(system_frame, light_frame);
    }

    #[test]
    fn hit_test_prioritizes_step_buttons_and_save_cancel() {
        let row = SettingField::PinLimit.row_rect();
        assert_eq!(
            SettingsPanel::hit_test(PixelPoint::new(row.right() - 70, row.origin.y + 20)),
            Some(SettingsPanelAction::Decrement)
        );
        assert_eq!(
            SettingsPanel::hit_test(PixelPoint::new(row.right() - 30, row.origin.y + 20)),
            Some(SettingsPanelAction::Increment)
        );
        assert_eq!(
            SettingsPanel::hit_test(PixelPoint::new(50, row.origin.y + 20)),
            Some(SettingsPanelAction::Select(SettingField::PinLimit))
        );
        assert_eq!(
            SettingsPanel::hit_test(PixelPoint::new(
                save_rect().origin.x + 4,
                save_rect().origin.y + 4
            )),
            Some(SettingsPanelAction::Save)
        );
        assert_eq!(
            SettingsPanel::hit_test(PixelPoint::new(
                cancel_rect().origin.x + 4,
                cancel_rect().origin.y + 4
            )),
            Some(SettingsPanelAction::Cancel)
        );
    }

    #[test]
    fn save_failure_preserves_draft_and_cancel_restores_original() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        panel.select(SettingField::PinLimit);
        panel.step(1);
        let draft = panel.draft();
        panel.mark_save_failed("internal: path contains private value");
        assert_eq!(panel.draft(), draft);
        assert!(
            matches!(panel.status(), SettingsPanelStatus::Error(message) if message == "internal")
        );
        panel.cancel();
        assert_eq!(panel.draft(), panel.original());
        assert!(!panel.is_dirty());
    }

    #[test]
    fn language_cycles_across_the_supported_presets() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        panel.select(SettingField::OcrLanguage);
        panel.step(-1);
        assert_eq!(panel.draft().ocr_language, OcrLanguage::SimplifiedChinese);
        panel.step(1);
        assert_eq!(panel.draft().ocr_language, OcrLanguage::Auto);
        assert_eq!(
            panel.value_label(SettingField::OcrLanguage),
            "AUTO".to_string()
        );
    }

    #[test]
    fn ocr_confidence_threshold_steps_across_the_full_valid_range() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        panel.select(SettingField::OcrConfidenceThreshold);

        panel.step(-1);
        assert_eq!(panel.draft().ocr_confidence_threshold, 55);
        assert_eq!(
            panel.value_label(SettingField::OcrConfidenceThreshold),
            "55%"
        );

        for _ in 0..20 {
            panel.step(1);
        }
        assert_eq!(panel.draft().ocr_confidence_threshold, 100);

        for _ in 0..25 {
            panel.step(-1);
        }
        assert_eq!(panel.draft().ocr_confidence_threshold, 0);
    }

    #[test]
    fn history_retention_steps_within_configured_day_bounds() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        panel.select(SettingField::HistoryRetentionDays);

        panel.step(-1);
        assert_eq!(panel.draft().history_retention_days, 29);
        assert_eq!(
            panel.value_label(SettingField::HistoryRetentionDays),
            "29 DAYS"
        );

        for _ in 0..4_000 {
            panel.step(1);
        }
        assert_eq!(
            panel.draft().history_retention_days,
            HISTORY_RETENTION_DAYS_MAX
        );
        for _ in 0..4_000 {
            panel.step(-1);
        }
        assert_eq!(
            panel.draft().history_retention_days,
            HISTORY_RETENTION_DAYS_MIN
        );
    }

    #[test]
    fn history_capacity_steps_within_configured_byte_bounds() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        panel.select(SettingField::HistoryMaxBytes);

        panel.step(-1);
        assert_eq!(panel.draft().history_max_bytes, 960 * 1024 * 1024);
        assert_eq!(panel.value_label(SettingField::HistoryMaxBytes), "960 MB");

        panel.step(1);
        assert_eq!(panel.draft().history_max_bytes, 1024 * 1024 * 1024);
        assert_eq!(panel.value_label(SettingField::HistoryMaxBytes), "1 GB");

        for _ in 0..2_000 {
            panel.step(1);
        }
        assert_eq!(panel.draft().history_max_bytes, HISTORY_MAX_BYTES_MAX);
        for _ in 0..2_000 {
            panel.step(-1);
        }
        assert_eq!(panel.draft().history_max_bytes, HISTORY_MAX_BYTES_MIN);
    }

    #[test]
    fn export_format_cycles_and_jpeg_quality_stays_within_valid_range() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        panel.select(SettingField::ExportFormat);

        panel.step(1);
        assert_eq!(panel.draft().export_format, ExportImageFormat::Jpeg);
        assert_eq!(panel.value_label(SettingField::ExportFormat), "JPEG");
        panel.step(1);
        assert_eq!(panel.draft().export_format, ExportImageFormat::WebP);
        assert_eq!(
            panel.value_label(SettingField::ExportFormat),
            "LOSSLESS WEBP"
        );

        panel.select(SettingField::JpegQuality);
        panel.step(-1);
        assert_eq!(panel.draft().jpeg_quality, 85);
        assert_eq!(panel.value_label(SettingField::JpegQuality), "85%");
        for _ in 0..30 {
            panel.step(-1);
        }
        assert_eq!(panel.draft().jpeg_quality, 1);
        for _ in 0..30 {
            panel.step(1);
        }
        assert_eq!(panel.draft().jpeg_quality, 100);
    }

    #[test]
    fn default_pin_level_uses_explicit_on_off_controls_and_arrow_keys() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        let row = SettingField::DefaultPinAlwaysOnTop.row_rect();
        panel.select(SettingField::DefaultPinAlwaysOnTop);

        assert_eq!(
            SettingsPanel::hit_test(PixelPoint::new(
                toggle_off_rect(row).origin.x + 4,
                toggle_off_rect(row).origin.y + 4,
            )),
            Some(SettingsPanelAction::Decrement)
        );
        assert_eq!(
            SettingsPanel::hit_test(PixelPoint::new(
                toggle_on_rect(row).origin.x + 4,
                toggle_on_rect(row).origin.y + 4,
            )),
            Some(SettingsPanelAction::Increment)
        );

        panel.handle_key(SettingsPanelKey::Left);
        assert!(!panel.draft().default_pin_always_on_top);
        assert_eq!(
            panel.value_label(SettingField::DefaultPinAlwaysOnTop),
            "OFF"
        );
        panel.handle_key(SettingsPanelKey::Right);
        assert!(panel.draft().default_pin_always_on_top);
        assert_eq!(panel.value_label(SettingField::DefaultPinAlwaysOnTop), "ON");
    }

    #[test]
    fn default_pin_level_save_failure_keeps_the_draft_and_cancel_restores_it() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        panel.select(SettingField::DefaultPinAlwaysOnTop);
        panel.apply_action(SettingsPanelAction::Decrement);
        assert!(!panel.draft().default_pin_always_on_top);

        panel.mark_save_failed("settings_save_failed");
        assert!(!panel.draft().default_pin_always_on_top);
        panel.cancel();

        assert!(panel.draft().default_pin_always_on_top);
        assert_eq!(panel.draft(), panel.original());
    }

    #[test]
    fn expanded_panel_keeps_default_pin_level_row_above_the_action_buttons() {
        assert!(SettingField::DefaultPinAlwaysOnTop.row_rect().bottom() <= save_rect().origin.y);
        assert!(SettingField::HistoryRetentionDays.row_rect().bottom() <= save_rect().origin.y);
        assert!(SettingField::HistoryMaxBytes.row_rect().bottom() <= save_rect().origin.y);
        assert!(SettingField::OcrConfidenceThreshold.row_rect().bottom() <= save_rect().origin.y);
        assert!(SettingField::ExportFormat.row_rect().bottom() <= save_rect().origin.y);
        assert!(SettingField::JpegQuality.row_rect().bottom() <= save_rect().origin.y);
        assert!(SettingField::FullDisplayHotkey.row_rect().bottom() <= save_rect().origin.y);
    }

    #[test]
    fn hotkey_recording_accepts_safe_binding_and_rejects_conflicts() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        panel.select(SettingField::RegionHotkey);
        assert_eq!(
            panel.handle_key(SettingsPanelKey::Enter),
            Some(SettingsPanelAction::StartHotkeyRecording)
        );
        panel.start_hotkey_recording();
        assert_eq!(panel.recording_field(), Some(SettingField::RegionHotkey));
        assert_eq!(
            panel.record_hotkey(REGION_SECONDARY_HOTKEY),
            Err("hotkey_conflict")
        );
        assert_eq!(panel.recording_field(), Some(SettingField::RegionHotkey));

        let binding = HotkeyBinding::new(HotkeyModifiers::CONTROL, HotkeyCode::KeyR);
        panel.record_hotkey(binding).expect("record safe hotkey");
        assert_eq!(panel.draft().region_hotkey, binding);
        assert_eq!(panel.recording_field(), None);
    }

    #[test]
    fn escape_cancels_hotkey_recording_without_mutating_draft() {
        let mut panel = SettingsPanel::new(AppSettings::default());
        let original = panel.draft().full_display_hotkey;
        panel.select(SettingField::FullDisplayHotkey);
        panel.start_hotkey_recording();
        panel.handle_key(SettingsPanelKey::Escape);

        assert_eq!(panel.recording_field(), None);
        assert_eq!(panel.draft().full_display_hotkey, original);
    }
}
