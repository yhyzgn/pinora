//! 辅助面板共享的无状态调色板与主题解析。
//!
//! 此模块不读取系统设置、环境变量或用户内容。窗口适配器只把 winit 已报告的
//! 外观转换为 [`SystemAppearance`]，再由这里稳定解析为自绘 XRGB 颜色 token。

use pinora_core::ThemeMode;
use winit::window::Theme;

/// 仅表示窗口系统 API 已明确报告的外观；未知状态不能猜测为浅色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemAppearance {
    Unknown,
    Light,
    Dark,
}

impl SystemAppearance {
    pub const fn from_winit(theme: Option<Theme>) -> Self {
        match theme {
            Some(Theme::Light) => Self::Light,
            Some(Theme::Dark) => Self::Dark,
            None => Self::Unknown,
        }
    }
}

/// 三种辅助面板共用的 XRGB 颜色 token。
///
/// 颜色仅服务设置、历史和诊断窗口，不能被 Overlay、贴图或原生菜单复用，以免
/// 在这些独立交互面上偷偷改变对比度和可读性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelTheme {
    pub background: u32,
    pub header: u32,
    pub border: u32,
    pub surface: u32,
    pub recessed_surface: u32,
    pub selected_surface: u32,
    pub control_surface: u32,
    pub control_border: u32,
    pub primary_text: u32,
    pub secondary_text: u32,
    pub muted_text: u32,
    pub accent_text: u32,
    pub on_action_text: u32,
    pub primary_action: u32,
    pub primary_action_border: u32,
    pub secondary_action: u32,
    pub secondary_action_border: u32,
    pub success_action: u32,
    pub success_action_border: u32,
    pub danger_action: u32,
    pub danger_action_border: u32,
    pub warning_action: u32,
    pub warning_action_border: u32,
    pub available_status: u32,
    pub restricted_status: u32,
}

impl PanelTheme {
    pub const fn resolve(preference: ThemeMode, system_appearance: SystemAppearance) -> Self {
        match preference {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
            ThemeMode::System => match system_appearance {
                SystemAppearance::Light => Self::light(),
                SystemAppearance::Unknown | SystemAppearance::Dark => Self::dark(),
            },
        }
    }

    pub const fn dark() -> Self {
        Self {
            background: 0x0014_1820,
            header: 0x0020_2B3A,
            border: 0x005A_718A,
            surface: 0x0020_2A36,
            recessed_surface: 0x001D_2630,
            selected_surface: 0x002B_5270,
            control_surface: 0x0034_4250,
            control_border: 0x007E_9AB2,
            primary_text: 0x00E5_EDF5,
            secondary_text: 0x00B5_D8FF,
            muted_text: 0x00A0_B2C4,
            accent_text: 0x009D_C4F0,
            on_action_text: 0x00FF_FFFF,
            primary_action: 0x002C_6EA3,
            primary_action_border: 0x0096_C8FF,
            secondary_action: 0x0043_4D59,
            secondary_action_border: 0x007A_8998,
            success_action: 0x002A_7C52,
            success_action_border: 0x0090_D79D,
            danger_action: 0x007A_3B3B,
            danger_action_border: 0x00E3_9A9A,
            warning_action: 0x006A_3D32,
            warning_action_border: 0x00E3_B18A,
            available_status: 0x007D_D7A0,
            restricted_status: 0x00F0_B777,
        }
    }

    pub const fn light() -> Self {
        Self {
            background: 0x00F4_F7FB,
            header: 0x00E6_EDF5,
            border: 0x0074_8AA2,
            surface: 0x00FF_FFFF,
            recessed_surface: 0x00EA_F0F6,
            selected_surface: 0x00C5_E4FA,
            control_surface: 0x00DD_E8F2,
            control_border: 0x0074_96AE,
            primary_text: 0x0017_273A,
            secondary_text: 0x002A_5E8E,
            muted_text: 0x0054_677B,
            accent_text: 0x0025_6FA7,
            on_action_text: 0x00FF_FFFF,
            primary_action: 0x002A_77B6,
            primary_action_border: 0x001D_5E91,
            secondary_action: 0x0061_7282,
            secondary_action_border: 0x0049_5A68,
            success_action: 0x002D_7C5A,
            success_action_border: 0x001F_5F42,
            danger_action: 0x00A9_4545,
            danger_action_border: 0x0084_3030,
            warning_action: 0x00A4_5E1E,
            warning_action_border: 0x0083_4814,
            available_status: 0x001F_6B4A,
            restricted_status: 0x0094_5015,
        }
    }
}

/// 某个辅助窗口的持久化偏好和最近已知系统外观。
///
/// 即使用户暂时强制浅色或深色，仍记录后续系统事件；再次切回 `System` 时会立即
/// 使用最近的明确系统外观，而不是等待下一次平台事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelThemeState {
    preference: ThemeMode,
    system_appearance: SystemAppearance,
}

impl PanelThemeState {
    pub const fn new(preference: ThemeMode, system_appearance: SystemAppearance) -> Self {
        Self {
            preference,
            system_appearance,
        }
    }

    pub const fn palette(self) -> PanelTheme {
        PanelTheme::resolve(self.preference, self.system_appearance)
    }

    pub fn set_preference(&mut self, preference: ThemeMode) -> bool {
        let changed = self.preference != preference;
        self.preference = preference;
        changed
    }

    /// 返回是否应重绘。强制浅/深色时仍吸收事件，但不产生无效帧。
    pub fn update_system_appearance(&mut self, appearance: SystemAppearance) -> bool {
        let changed = self.system_appearance != appearance;
        self.system_appearance = appearance;
        changed && self.preference == ThemeMode::System
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_preferences_override_system_appearance() {
        assert_eq!(
            PanelTheme::resolve(ThemeMode::Light, SystemAppearance::Dark),
            PanelTheme::light()
        );
        assert_eq!(
            PanelTheme::resolve(ThemeMode::Dark, SystemAppearance::Light),
            PanelTheme::dark()
        );
    }

    #[test]
    fn system_unknown_stably_uses_dark_palette() {
        assert_eq!(
            PanelTheme::resolve(ThemeMode::System, SystemAppearance::Unknown),
            PanelTheme::dark()
        );
        assert_ne!(PanelTheme::light(), PanelTheme::dark());
    }

    #[test]
    fn system_event_repaints_only_for_system_preference() {
        let mut state = PanelThemeState::new(ThemeMode::Light, SystemAppearance::Dark);
        assert!(!state.update_system_appearance(SystemAppearance::Light));
        assert_eq!(state.palette(), PanelTheme::light());

        assert!(state.set_preference(ThemeMode::System));
        assert_eq!(state.palette(), PanelTheme::light());
        assert!(state.update_system_appearance(SystemAppearance::Dark));
        assert_eq!(state.palette(), PanelTheme::dark());
    }
}
