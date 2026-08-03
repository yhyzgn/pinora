//! 捕获后端选择：KDE/Spectacle 优先，其次 xcap；无真实后端时进入受限状态。
//!
//! 重要：在 KDE Wayland 上 **不要** 默认走 xcap→portal/PipeWire。
//! 那条路径会卡数秒；Snipaste/飞书/微信在 Windows 用的是原生 GDI/DXGI，
//! 在 KDE 上对标的是 KWin 内部截图（Spectacle / ScreenShot2），不是 portal。

use pinora_core::{
    CaptureImage, CaptureProvider, CaptureRequest, CaptureWindowInfo, DisplayInfo, ErrorCode,
    PinoraError,
};

use super::capture_fake::FakeCaptureProvider;
use super::capture_kde::KdeSpectacleCaptureProvider;
use super::capture_xcap::XcapCaptureProvider;

/// 当前选用的捕获后端名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackendKind {
    /// KDE Spectacle → KWin（快，本机实测 ~0.5s）
    Kde,
    /// xcap（X11 快；Wayland 常走 portal，慢）
    Xcap,
    /// 没有可用的真实截图后端。
    Unavailable,
    Fake,
}

impl CaptureBackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kde => "kde-spectacle",
            Self::Xcap => "xcap",
            Self::Unavailable => "unavailable",
            Self::Fake => "fake",
        }
    }
}

/// 可在真实后端、受限状态和显式测试 fake 之间切换的捕获提供者。
#[derive(Debug, Clone)]
pub enum SelectedCaptureProvider {
    Kde(KdeSpectacleCaptureProvider),
    Xcap(XcapCaptureProvider),
    Unavailable { reason: String },
    Fake(FakeCaptureProvider),
}

impl SelectedCaptureProvider {
    /// 自动探测顺序：KDE Spectacle → xcap → 受限状态。
    pub fn autodetect() -> (Self, CaptureBackendKind, Option<String>) {
        let kde = KdeSpectacleCaptureProvider::probe_available();
        let xcap = if kde.is_err() {
            Some(XcapCaptureProvider::probe_available())
        } else {
            None
        };
        select_from_probes(kde, xcap)
    }

    pub fn kind(&self) -> CaptureBackendKind {
        match self {
            Self::Kde(_) => CaptureBackendKind::Kde,
            Self::Xcap(_) => CaptureBackendKind::Xcap,
            Self::Unavailable { .. } => CaptureBackendKind::Unavailable,
            Self::Fake(_) => CaptureBackendKind::Fake,
        }
    }

    pub fn primary_display_id(&self) -> pinora_core::DisplayId {
        match self {
            Self::Fake(f) => f.primary_display_id(),
            Self::Unavailable { .. } => pinora_core::DisplayId::new("unavailable"),
            Self::Kde(k) => k
                .displays()
                .ok()
                .and_then(|d| d.into_iter().next().map(|i| i.id))
                .unwrap_or_else(|| pinora_core::DisplayId::new("kde-unknown")),
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
            Self::Kde(p) => p.displays(),
            Self::Xcap(p) => p.displays(),
            Self::Unavailable { reason } => Err(PinoraError::new(
                ErrorCode::CapabilityUnavailable,
                reason.clone(),
            )),
            Self::Fake(p) => p.displays(),
        }
    }

    fn windows(&self) -> Result<Vec<CaptureWindowInfo>, PinoraError> {
        match self {
            Self::Kde(p) => p.windows(),
            Self::Xcap(p) => p.windows(),
            Self::Unavailable { reason } => Err(PinoraError::new(
                ErrorCode::CapabilityUnavailable,
                reason.clone(),
            )),
            Self::Fake(p) => p.windows(),
        }
    }

    fn capture(&self, request: CaptureRequest) -> Result<CaptureImage, PinoraError> {
        match self {
            Self::Kde(p) => p.capture(request),
            Self::Xcap(p) => p.capture(request),
            Self::Unavailable { reason } => Err(PinoraError::new(
                ErrorCode::CapabilityUnavailable,
                reason.clone(),
            )),
            Self::Fake(p) => p.capture(request),
        }
    }
}

fn select_from_probes(
    kde: Result<KdeSpectacleCaptureProvider, PinoraError>,
    xcap: Option<Result<Vec<DisplayInfo>, PinoraError>>,
) -> (SelectedCaptureProvider, CaptureBackendKind, Option<String>) {
    match kde {
        Ok(provider) => {
            let displays = provider.displays().unwrap_or_default();
            let note = if let Some(d0) = displays.iter().max_by_key(|d| d.bounds.size.area()) {
                format!(
                    "kde/spectacle monitors={} primary={} {}x{} (~0.5s KWin path, not portal)",
                    displays.len(),
                    d0.name,
                    d0.bounds.size.width,
                    d0.bounds.size.height
                )
            } else {
                "kde/spectacle available".into()
            };
            (
                SelectedCaptureProvider::Kde(provider),
                CaptureBackendKind::Kde,
                Some(note),
            )
        }
        Err(kde_err) => match xcap {
            Some(Ok(displays)) if !displays.is_empty() => {
                let d0 = &displays[0];
                (
                    SelectedCaptureProvider::Xcap(XcapCaptureProvider::new()),
                    CaptureBackendKind::Xcap,
                    Some(format!(
                        "xcap fallback (kde unavailable: {kde_err}); monitors={} primary={} {}x{} — Wayland 上可能很慢",
                        displays.len(),
                        d0.name,
                        d0.bounds.size.width,
                        d0.bounds.size.height
                    )),
                )
            }
            Some(Ok(_)) => unavailable_selection(format!(
                "capture unavailable: kde unavailable ({kde_err}); xcap reported no monitors"
            )),
            Some(Err(xcap_err)) => unavailable_selection(format!(
                "capture unavailable: kde unavailable ({kde_err}); xcap unavailable ({xcap_err})"
            )),
            None => unavailable_selection(format!(
                "capture unavailable: kde unavailable ({kde_err}); xcap probe was not run"
            )),
        },
    }
}

fn unavailable_selection(
    reason: String,
) -> (SelectedCaptureProvider, CaptureBackendKind, Option<String>) {
    (
        SelectedCaptureProvider::Unavailable {
            reason: reason.clone(),
        },
        CaptureBackendKind::Unavailable,
        Some(reason),
    )
}

/// 强制使用 fake（测试）。
pub fn fake_only() -> SelectedCaptureProvider {
    SelectedCaptureProvider::Fake(FakeCaptureProvider::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unavailable(message: &str) -> PinoraError {
        PinoraError::new(ErrorCode::CapabilityUnavailable, message)
    }

    fn sample_display() -> DisplayInfo {
        DisplayInfo {
            id: pinora_core::DisplayId::new("test-display"),
            name: "Test display".into(),
            bounds: pinora_core::PixelRect::new(0, 0, 1920, 1080),
            scale: 1.0,
        }
    }

    #[test]
    fn fake_only_backend() {
        let p = fake_only();
        assert_eq!(p.kind(), CaptureBackendKind::Fake);
        assert!(!p.displays().unwrap().is_empty());
    }

    #[test]
    fn kde_probe_wins_over_xcap() {
        let kde = KdeSpectacleCaptureProvider::new("/unused/spectacle".into());
        let (selected, kind, _) = select_from_probes(Ok(kde), None);

        assert_eq!(kind, CaptureBackendKind::Kde);
        assert!(matches!(selected, SelectedCaptureProvider::Kde(_)));
    }

    #[test]
    fn xcap_is_selected_when_kde_is_unavailable() {
        let (selected, kind, _) = select_from_probes(
            Err(unavailable("kde unavailable")),
            Some(Ok(vec![sample_display()])),
        );

        assert_eq!(kind, CaptureBackendKind::Xcap);
        assert!(matches!(selected, SelectedCaptureProvider::Xcap(_)));
    }

    #[test]
    fn no_real_backend_enters_unavailable_state_without_fake() {
        let (selected, kind, note) = select_from_probes(
            Err(unavailable("kde unavailable")),
            Some(Err(unavailable("xcap unavailable"))),
        );

        assert_eq!(kind, CaptureBackendKind::Unavailable);
        assert!(note.expect("diagnostic note").contains("xcap unavailable"));
        assert!(matches!(
            selected,
            SelectedCaptureProvider::Unavailable { .. }
        ));
        assert_eq!(
            selected.displays().unwrap_err().code,
            ErrorCode::CapabilityUnavailable
        );
        let capture_error = selected
            .capture(CaptureRequest::FullDisplay {
                display: pinora_core::DisplayId::new("unavailable"),
            })
            .expect_err("unavailable provider must never create an image");
        assert_eq!(capture_error.code, ErrorCode::CapabilityUnavailable);
    }
}
