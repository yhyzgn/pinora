//! 版本化本地设置的纯领域模型。

/// 当前设置 schema 版本。
pub const SETTINGS_SCHEMA_VERSION: u16 = 5;
pub const DEFAULT_HISTORY_LIMIT: u32 = 100;
pub const DEFAULT_PIN_LIMIT: u16 = 10;
pub const DEFAULT_PIN_OPACITY_PERCENT: u8 = 100;
pub const DEFAULT_PIN_ALWAYS_ON_TOP: bool = true;
pub const DEFAULT_OCR_CONFIDENCE_THRESHOLD: u8 = 60;

/// 被设置持久化支持的跨平台物理键。
///
/// 该集合刻意小于平台 SDK 的完整键表：保存的组合须能在 Windows、macOS 和
/// Linux X11 的现有后端中稳定映射，未知键不会被写入配置文件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyCode {
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
}

impl HotkeyCode {
    pub const fn to_wire(self) -> u8 {
        match self {
            Self::F1 => 1,
            Self::F2 => 2,
            Self::F3 => 3,
            Self::F4 => 4,
            Self::F5 => 5,
            Self::F6 => 6,
            Self::F7 => 7,
            Self::F8 => 8,
            Self::F9 => 9,
            Self::F10 => 10,
            Self::F11 => 11,
            Self::F12 => 12,
            Self::KeyA => 32,
            Self::KeyB => 33,
            Self::KeyC => 34,
            Self::KeyD => 35,
            Self::KeyE => 36,
            Self::KeyF => 37,
            Self::KeyG => 38,
            Self::KeyH => 39,
            Self::KeyI => 40,
            Self::KeyJ => 41,
            Self::KeyK => 42,
            Self::KeyL => 43,
            Self::KeyM => 44,
            Self::KeyN => 45,
            Self::KeyO => 46,
            Self::KeyP => 47,
            Self::KeyQ => 48,
            Self::KeyR => 49,
            Self::KeyS => 50,
            Self::KeyT => 51,
            Self::KeyU => 52,
            Self::KeyV => 53,
            Self::KeyW => 54,
            Self::KeyX => 55,
            Self::KeyY => 56,
            Self::KeyZ => 57,
        }
    }

    pub const fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::F1),
            2 => Some(Self::F2),
            3 => Some(Self::F3),
            4 => Some(Self::F4),
            5 => Some(Self::F5),
            6 => Some(Self::F6),
            7 => Some(Self::F7),
            8 => Some(Self::F8),
            9 => Some(Self::F9),
            10 => Some(Self::F10),
            11 => Some(Self::F11),
            12 => Some(Self::F12),
            32 => Some(Self::KeyA),
            33 => Some(Self::KeyB),
            34 => Some(Self::KeyC),
            35 => Some(Self::KeyD),
            36 => Some(Self::KeyE),
            37 => Some(Self::KeyF),
            38 => Some(Self::KeyG),
            39 => Some(Self::KeyH),
            40 => Some(Self::KeyI),
            41 => Some(Self::KeyJ),
            42 => Some(Self::KeyK),
            43 => Some(Self::KeyL),
            44 => Some(Self::KeyM),
            45 => Some(Self::KeyN),
            46 => Some(Self::KeyO),
            47 => Some(Self::KeyP),
            48 => Some(Self::KeyQ),
            49 => Some(Self::KeyR),
            50 => Some(Self::KeyS),
            51 => Some(Self::KeyT),
            52 => Some(Self::KeyU),
            53 => Some(Self::KeyV),
            54 => Some(Self::KeyW),
            55 => Some(Self::KeyX),
            56 => Some(Self::KeyY),
            57 => Some(Self::KeyZ),
            _ => None,
        }
    }

    pub const fn is_function_key(self) -> bool {
        matches!(
            self,
            Self::F1
                | Self::F2
                | Self::F3
                | Self::F4
                | Self::F5
                | Self::F6
                | Self::F7
                | Self::F8
                | Self::F9
                | Self::F10
                | Self::F11
                | Self::F12
        )
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
            Self::KeyA => "A",
            Self::KeyB => "B",
            Self::KeyC => "C",
            Self::KeyD => "D",
            Self::KeyE => "E",
            Self::KeyF => "F",
            Self::KeyG => "G",
            Self::KeyH => "H",
            Self::KeyI => "I",
            Self::KeyJ => "J",
            Self::KeyK => "K",
            Self::KeyL => "L",
            Self::KeyM => "M",
            Self::KeyN => "N",
            Self::KeyO => "O",
            Self::KeyP => "P",
            Self::KeyQ => "Q",
            Self::KeyR => "R",
            Self::KeyS => "S",
            Self::KeyT => "T",
            Self::KeyU => "U",
            Self::KeyV => "V",
            Self::KeyW => "W",
            Self::KeyX => "X",
            Self::KeyY => "Y",
            Self::KeyZ => "Z",
        }
    }
}

/// 跨平台热键修饰键位图。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HotkeyModifiers(u8);

impl HotkeyModifiers {
    pub const NONE: Self = Self(0);
    pub const CONTROL: Self = Self(1);
    pub const ALT: Self = Self(1 << 1);
    pub const SHIFT: Self = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);
    const KNOWN_BITS: u8 = Self::CONTROL.0 | Self::ALT.0 | Self::SHIFT.0 | Self::SUPER.0;

    pub const fn from_wire(value: u8) -> Option<Self> {
        if value & !Self::KNOWN_BITS == 0 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn to_wire(self) -> u8 {
        self.0
    }

    pub const fn contains(self, modifier: Self) -> bool {
        self.0 & modifier.0 == modifier.0
    }

    pub const fn has_non_shift_modifier(self) -> bool {
        self.0 & (Self::CONTROL.0 | Self::ALT.0 | Self::SUPER.0) != 0
    }
}

impl std::ops::BitOr for HotkeyModifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// 一个可持久化、可映射的全局热键组合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HotkeyBinding {
    pub modifiers: HotkeyModifiers,
    pub code: HotkeyCode,
}

impl HotkeyBinding {
    pub const fn new(modifiers: HotkeyModifiers, code: HotkeyCode) -> Self {
        Self { modifiers, code }
    }

    /// 字母键必须至少带 Ctrl、Alt 或 Super；功能键可独立使用。
    pub const fn is_safe(self) -> bool {
        self.code.is_function_key() || self.modifiers.has_non_shift_modifier()
    }
}

impl std::fmt::Display for HotkeyBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut needs_separator = false;
        for (modifier, label) in [
            (HotkeyModifiers::CONTROL, "Ctrl"),
            (HotkeyModifiers::ALT, "Alt"),
            (HotkeyModifiers::SHIFT, "Shift"),
            (HotkeyModifiers::SUPER, "Super"),
        ] {
            if self.modifiers.contains(modifier) {
                if needs_separator {
                    formatter.write_str("+")?;
                }
                formatter.write_str(label)?;
                needs_separator = true;
            }
        }
        if needs_separator {
            formatter.write_str("+")?;
        }
        formatter.write_str(self.code.label())
    }
}

pub const DEFAULT_REGION_HOTKEY: HotkeyBinding =
    HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F2);
pub const DEFAULT_FULL_DISPLAY_HOTKEY: HotkeyBinding =
    HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F3);
pub const REGION_SECONDARY_HOTKEY: HotkeyBinding =
    HotkeyBinding::new(HotkeyModifiers::CONTROL, HotkeyCode::KeyN);
pub const REGION_ALTERNATE_HOTKEY: HotkeyBinding = HotkeyBinding::new(
    HotkeyModifiers(HotkeyModifiers::CONTROL.0 | HotkeyModifiers::SHIFT.0),
    HotkeyCode::KeyS,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub const fn to_wire(self) -> u8 {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }

    pub const fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::System),
            1 => Some(Self::Light),
            2 => Some(Self::Dark),
            _ => None,
        }
    }
}

/// 本地 Tesseract OCR 的稳定语言预设。
///
/// 预设不是任意 CLI 参数：应用只会从本机已安装模型中解析它们，避免设置内容
/// 变成外部进程参数或隐式下载请求。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrLanguage {
    Auto,
    English,
    SimplifiedChinese,
}

impl OcrLanguage {
    pub const fn to_wire(self) -> u8 {
        match self {
            Self::Auto => 0,
            Self::English => 1,
            Self::SimplifiedChinese => 2,
        }
    }

    pub const fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Auto),
            1 => Some(Self::English),
            2 => Some(Self::SimplifiedChinese),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppSettings {
    pub schema_version: u16,
    pub theme: ThemeMode,
    pub history_limit: u32,
    pub pin_limit: u16,
    pub default_pin_opacity_percent: u8,
    pub default_pin_always_on_top: bool,
    pub ocr_language: OcrLanguage,
    /// 低于此百分比的已知 OCR 词仅在文字层中使用告警样式呈现。
    pub ocr_confidence_threshold: u8,
    pub region_hotkey: HotkeyBinding,
    pub full_display_hotkey: HotkeyBinding,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            theme: ThemeMode::System,
            history_limit: DEFAULT_HISTORY_LIMIT,
            pin_limit: DEFAULT_PIN_LIMIT,
            default_pin_opacity_percent: DEFAULT_PIN_OPACITY_PERCENT,
            default_pin_always_on_top: DEFAULT_PIN_ALWAYS_ON_TOP,
            ocr_language: OcrLanguage::Auto,
            ocr_confidence_threshold: DEFAULT_OCR_CONFIDENCE_THRESHOLD,
            region_hotkey: DEFAULT_REGION_HOTKEY,
            full_display_hotkey: DEFAULT_FULL_DISPLAY_HOTKEY,
        }
    }
}

/// 表示读取时发生的逐字段修复，不携带文件路径或用户内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SettingsRepairs {
    /// v1 设置成功按默认新增后续字段；下次保存会原子替换为当前记录。
    pub migrated_from_v1: bool,
    /// v2 设置成功按默认新增 v3 热键字段；下次保存会原子替换为当前记录。
    pub migrated_from_v2: bool,
    /// v3 设置成功按默认新增贴图默认置顶字段；下次保存会原子替换为当前记录。
    pub migrated_from_v3: bool,
    /// v4 设置成功按默认新增 OCR 置信度阈值字段；下次保存会原子替换为当前记录。
    pub migrated_from_v4: bool,
    pub history_limit: bool,
    pub pin_limit: bool,
    pub default_pin_opacity_percent: bool,
    pub ocr_confidence_threshold: bool,
    pub region_hotkey: bool,
    pub full_display_hotkey: bool,
}

impl SettingsRepairs {
    pub const fn is_empty(self) -> bool {
        !self.migrated_from_v1
            && !self.migrated_from_v2
            && !self.migrated_from_v3
            && !self.migrated_from_v4
            && !self.history_limit
            && !self.pin_limit
            && !self.default_pin_opacity_percent
            && !self.ocr_confidence_threshold
            && !self.region_hotkey
            && !self.full_display_hotkey
    }
}

impl AppSettings {
    /// 仅修复数值字段；schema 与枚举值必须先由 codec 严格验证。
    pub fn with_repaired_values(mut self) -> (Self, SettingsRepairs) {
        let mut repairs = SettingsRepairs::default();
        if !(1..=10_000).contains(&self.history_limit) {
            self.history_limit = DEFAULT_HISTORY_LIMIT;
            repairs.history_limit = true;
        }
        if !(1..=100).contains(&self.pin_limit) {
            self.pin_limit = DEFAULT_PIN_LIMIT;
            repairs.pin_limit = true;
        }
        if !(15..=100).contains(&self.default_pin_opacity_percent) {
            self.default_pin_opacity_percent = DEFAULT_PIN_OPACITY_PERCENT;
            repairs.default_pin_opacity_percent = true;
        }
        if self.ocr_confidence_threshold > 100 {
            self.ocr_confidence_threshold = DEFAULT_OCR_CONFIDENCE_THRESHOLD;
            repairs.ocr_confidence_threshold = true;
        }
        if !self.region_hotkey.is_safe()
            || self.region_hotkey == REGION_SECONDARY_HOTKEY
            || self.region_hotkey == REGION_ALTERNATE_HOTKEY
        {
            self.region_hotkey = DEFAULT_REGION_HOTKEY;
            repairs.region_hotkey = true;
        }
        if !self.full_display_hotkey.is_safe()
            || self.full_display_hotkey == self.region_hotkey
            || self.full_display_hotkey == REGION_SECONDARY_HOTKEY
            || self.full_display_hotkey == REGION_ALTERNATE_HOTKEY
        {
            self.full_display_hotkey = DEFAULT_FULL_DISPLAY_HOTKEY;
            repairs.full_display_hotkey = true;
        }
        (self, repairs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_current_and_valid() {
        let settings = AppSettings::default();
        let (repaired, repairs) = settings.with_repaired_values();
        assert_eq!(settings, repaired);
        assert!(repairs.is_empty());
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
    }

    #[test]
    fn invalid_numeric_values_fall_back_per_field() {
        let (settings, repairs) = AppSettings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            theme: ThemeMode::Dark,
            history_limit: 0,
            pin_limit: 101,
            default_pin_opacity_percent: 14,
            default_pin_always_on_top: false,
            ocr_language: OcrLanguage::English,
            ocr_confidence_threshold: u8::MAX,
            region_hotkey: HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::KeyA),
            full_display_hotkey: HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F2),
        }
        .with_repaired_values();

        assert_eq!(settings.history_limit, DEFAULT_HISTORY_LIMIT);
        assert_eq!(settings.pin_limit, DEFAULT_PIN_LIMIT);
        assert_eq!(
            settings.default_pin_opacity_percent,
            DEFAULT_PIN_OPACITY_PERCENT
        );
        assert!(repairs.history_limit);
        assert!(repairs.pin_limit);
        assert!(repairs.default_pin_opacity_percent);
        assert_eq!(
            settings.ocr_confidence_threshold,
            DEFAULT_OCR_CONFIDENCE_THRESHOLD
        );
        assert!(repairs.ocr_confidence_threshold);
    }

    #[test]
    fn ocr_language_wire_values_are_stable() {
        assert_eq!(OcrLanguage::Auto.to_wire(), 0);
        assert_eq!(OcrLanguage::English.to_wire(), 1);
        assert_eq!(OcrLanguage::SimplifiedChinese.to_wire(), 2);
        assert_eq!(
            OcrLanguage::from_wire(2),
            Some(OcrLanguage::SimplifiedChinese)
        );
        assert_eq!(OcrLanguage::from_wire(3), None);
    }

    #[test]
    fn hotkey_display_and_safety_are_stable() {
        let binding = HotkeyBinding::new(
            HotkeyModifiers::CONTROL | HotkeyModifiers::SHIFT,
            HotkeyCode::KeyS,
        );
        assert_eq!(binding.to_string(), "Ctrl+Shift+S");
        assert!(binding.is_safe());
        assert!(!HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::KeyA).is_safe());
        assert!(HotkeyBinding::new(HotkeyModifiers::NONE, HotkeyCode::F12).is_safe());
    }

    #[test]
    fn invalid_or_conflicting_hotkeys_are_repaired_individually() {
        let (settings, repairs) = AppSettings {
            region_hotkey: REGION_SECONDARY_HOTKEY,
            full_display_hotkey: DEFAULT_REGION_HOTKEY,
            ..AppSettings::default()
        }
        .with_repaired_values();

        assert_eq!(settings.region_hotkey, DEFAULT_REGION_HOTKEY);
        assert_eq!(settings.full_display_hotkey, DEFAULT_FULL_DISPLAY_HOTKEY);
        assert!(repairs.region_hotkey);
        assert!(repairs.full_display_hotkey);
    }
}
