//! 捕获后端选择：优先 xcap，失败降级 fake。

use pinora_core::{CaptureImage, CaptureProvider, CaptureRequest, DisplayInfo, PinoraError};

use crate::capture_fake::FakeCaptureProvider;
use crate::capture_xcap::XcapCaptureProvider;

/// 当前选用的捕获后端名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackendKind {
    Xcap,
    Fake,
}

impl CaptureBackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Xcap => "xcap",
            Self::Fake => "fake",
        }
    }
}

/// 可在 xcap / fake 之间切换的捕获提供者。
#[derive(Debug, Clone)]
pub enum SelectedCaptureProvider {
    Xcap(XcapCaptureProvider),
    Fake(FakeCaptureProvider),
}

impl SelectedCaptureProvider {
    /// 探测 xcap（仅枚举显示器，不做试截图，避免启动多等数秒）。
    pub fn autodetect() -> (Self, CaptureBackendKind, Option<String>) {
        match XcapCaptureProvider::probe_available() {
            Ok(displays) if !displays.is_empty() => {
                let d0 = &displays[0];
                (
                    Self::Xcap(XcapCaptureProvider::new()),
                    CaptureBackendKind::Xcap,
                    Some(format!(
                        "xcap monitors={} primary={} {}x{} (capture on confirm)",
                        displays.len(),
                        d0.name,
                        d0.bounds.size.width,
                        d0.bounds.size.height
                    )),
                )
            }
            Ok(_) => (
                Self::Fake(FakeCaptureProvider::new()),
                CaptureBackendKind::Fake,
                Some("xcap returned no monitors; using fake".into()),
            ),
            Err(err) => (
                Self::Fake(FakeCaptureProvider::new()),
                CaptureBackendKind::Fake,
                Some(format!("xcap unavailable ({err}); using fake")),
            ),
        }
    }

    pub fn kind(&self) -> CaptureBackendKind {
        match self {
            Self::Xcap(_) => CaptureBackendKind::Xcap,
            Self::Fake(_) => CaptureBackendKind::Fake,
        }
    }

    pub fn primary_display_id(&self) -> pinora_core::DisplayId {
        match self {
            Self::Fake(f) => f.primary_display_id(),
            Self::Xcap(x) => x
                .displays()
                .ok()
                .and_then(|d| d.into_iter().next().map(|i| i.id))
                .unwrap_or_else(|| pinora_core::DisplayId::new("xcap-unknown")),
        }
    }
}

impl CaptureProvider for SelectedCaptureProvider {
    fn displays(&self) -> Result<Vec<DisplayInfo>, PinoraError> {
        match self {
            Self::Xcap(p) => p.displays(),
            Self::Fake(p) => p.displays(),
        }
    }

    fn capture(&self, request: CaptureRequest) -> Result<CaptureImage, PinoraError> {
        match self {
            Self::Xcap(p) => p.capture(request),
            Self::Fake(p) => p.capture(request),
        }
    }
}

/// 强制使用 fake（测试）。
pub fn fake_only() -> SelectedCaptureProvider {
    SelectedCaptureProvider::Fake(FakeCaptureProvider::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_only_backend() {
        let p = fake_only();
        assert_eq!(p.kind(), CaptureBackendKind::Fake);
        assert!(!p.displays().unwrap().is_empty());
    }
}
