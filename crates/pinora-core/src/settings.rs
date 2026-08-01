//! 版本化本地设置的纯领域模型。

/// 当前设置 schema 版本。
pub const SETTINGS_SCHEMA_VERSION: u16 = 1;
pub const DEFAULT_HISTORY_LIMIT: u32 = 100;
pub const DEFAULT_PIN_LIMIT: u16 = 10;
pub const DEFAULT_PIN_OPACITY_PERCENT: u8 = 100;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppSettings {
    pub schema_version: u16,
    pub theme: ThemeMode,
    pub history_limit: u32,
    pub pin_limit: u16,
    pub default_pin_opacity_percent: u8,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            theme: ThemeMode::System,
            history_limit: DEFAULT_HISTORY_LIMIT,
            pin_limit: DEFAULT_PIN_LIMIT,
            default_pin_opacity_percent: DEFAULT_PIN_OPACITY_PERCENT,
        }
    }
}

/// 表示读取时发生的逐字段修复，不携带文件路径或用户内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SettingsRepairs {
    pub history_limit: bool,
    pub pin_limit: bool,
    pub default_pin_opacity_percent: bool,
}

impl SettingsRepairs {
    pub const fn is_empty(self) -> bool {
        !self.history_limit && !self.pin_limit && !self.default_pin_opacity_percent
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
    }
}
