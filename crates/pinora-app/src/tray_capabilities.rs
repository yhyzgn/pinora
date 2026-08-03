//! tray 中展示的本次启动能力摘要。
//!
//! 菜单文本仅由明确的能力布尔值和固定标签生成。不要把 runtime notes、路径、
//! 平台后端错误、OCR 文本或剪贴板内容带入这里。

use pinora_core::CapabilitySnapshot;

pub(crate) const CAPABILITY_MENU_TITLE: &str = "环境能力（本次启动）";

/// 可安全显示在 tray 中的启动能力快照。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TrayCapabilitySummary {
    capture_available: bool,
    global_hotkey_available: bool,
    clipboard_image_available: bool,
    ocr_available: bool,
}

impl TrayCapabilitySummary {
    /// 热键状态必须来自实际注册结果，不能沿用 bootstrap probe 的平台猜测。
    pub(crate) const fn from_runtime(
        runtime: &CapabilitySnapshot,
        global_hotkey_available: bool,
        ocr_available: bool,
    ) -> Self {
        Self {
            capture_available: runtime.capture_available,
            global_hotkey_available,
            clipboard_image_available: runtime.clipboard_image_available,
            ocr_available,
        }
    }

    /// 标题下的只读菜单项，顺序固定以便快速扫描。
    pub(crate) const fn labels(self) -> [&'static str; 4] {
        [
            capture_label(self.capture_available),
            global_hotkey_label(self.global_hotkey_available),
            clipboard_label(self.clipboard_image_available),
            ocr_label(self.ocr_available),
        ]
    }
}

const fn capture_label(available: bool) -> &'static str {
    if available {
        "截图：可用"
    } else {
        "截图：受限"
    }
}

pub(crate) const fn global_hotkey_label(available: bool) -> &'static str {
    if available {
        "全局热键：可用"
    } else {
        "全局热键：受限"
    }
}

const fn clipboard_label(available: bool) -> &'static str {
    if available {
        "系统图像剪贴板：可用"
    } else {
        "系统图像剪贴板：受限"
    }
}

const fn ocr_label(available: bool) -> &'static str {
    if available {
        "本地 OCR：可用"
    } else {
        "本地 OCR：受限"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_hotkey_result_overrides_runtime_platform_guess() {
        let runtime = CapabilitySnapshot {
            capture_available: true,
            global_hotkey_available: true,
            clipboard_image_available: true,
            always_on_top_available: false,
            notes: vec!["/home/neo/private backend detail".into()],
        };

        let summary = TrayCapabilitySummary::from_runtime(&runtime, false, true);

        assert_eq!(
            summary.labels(),
            [
                "截图：可用",
                "全局热键：受限",
                "系统图像剪贴板：可用",
                "本地 OCR：可用",
            ]
        );
    }

    #[test]
    fn labels_are_fixed_and_never_expose_runtime_notes() {
        let runtime = CapabilitySnapshot {
            capture_available: false,
            global_hotkey_available: false,
            clipboard_image_available: false,
            always_on_top_available: false,
            notes: vec!["secret OCR text /tmp/private.png".into()],
        };
        let labels = TrayCapabilitySummary::from_runtime(&runtime, false, false).labels();

        assert_eq!(
            labels,
            [
                "截图：受限",
                "全局热键：受限",
                "系统图像剪贴板：受限",
                "本地 OCR：受限",
            ]
        );
        for label in labels {
            assert!(!label.contains("secret"));
            assert!(!label.contains("/tmp/"));
            assert!(!label.contains('\n'));
        }
    }
}
