//! 用户动作标识（热键、托盘、命令面板最终都映射到此）。

use std::fmt;

/// 稳定动作 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionId {
    /// 区域捕获并贴图（默认演示动作用固定区域）。
    CaptureRegionAndPin,
    /// 捕获当前目标显示器的完整图像。
    CaptureFullDisplay,
    /// 将最近一次捕获保存为 PNG。
    SaveLastCapture,
    /// 将最近一次捕获复制到剪贴板端口。
    CopyLastCapture,
    /// 将系统剪贴板中的图像创建为贴图。
    PasteClipboard,
    /// 切换全部贴图的可见性。
    ToggleAllPinsVisibility,
    /// 请求退出。
    Quit,
}

impl ActionId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CaptureRegionAndPin => "capture_region_and_pin",
            Self::CaptureFullDisplay => "capture_full_display",
            Self::SaveLastCapture => "save_last_capture",
            Self::CopyLastCapture => "copy_last_capture",
            Self::PasteClipboard => "paste_clipboard",
            Self::ToggleAllPinsVisibility => "toggle_all_pins_visibility",
            Self::Quit => "quit",
        }
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 热键绑定描述（平台无关字符串，后端自行解析）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub action: ActionId,
    /// 例如 `Ctrl+Shift+A`；本阶段仅存储，不解析。
    pub combo: String,
}

impl KeyBinding {
    pub fn new(action: ActionId, combo: impl Into<String>) -> Self {
        Self {
            action,
            combo: combo.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ids_are_stable() {
        assert_eq!(ActionId::Quit.as_str(), "quit");
        assert_eq!(
            ActionId::CaptureRegionAndPin.as_str(),
            "capture_region_and_pin"
        );
        assert_eq!(
            ActionId::CaptureFullDisplay.as_str(),
            "capture_full_display"
        );
        assert_eq!(ActionId::PasteClipboard.as_str(), "paste_clipboard");
        assert_eq!(
            ActionId::ToggleAllPinsVisibility.as_str(),
            "toggle_all_pins_visibility"
        );
    }
}
