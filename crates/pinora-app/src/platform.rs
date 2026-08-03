use pinora_core::CapabilitySnapshot;

use pinora_capture::CaptureBackendKind;

/// 平台能力探测。
pub trait CapabilityProbe {
    fn probe(&self) -> CapabilitySnapshot;
}

/// 根据所选捕获后端生成能力快照。
#[derive(Debug, Clone)]
pub struct RuntimeCapabilityProbe {
    pub capture_backend: CaptureBackendKind,
    pub capture_note: Option<String>,
}

impl RuntimeCapabilityProbe {
    pub fn new(capture_backend: CaptureBackendKind, capture_note: Option<String>) -> Self {
        Self {
            capture_backend,
            capture_note,
        }
    }
}

impl CapabilityProbe for RuntimeCapabilityProbe {
    fn probe(&self) -> CapabilitySnapshot {
        let mut notes = Vec::new();
        if let Some(note) = &self.capture_note {
            notes.push(note.clone());
        }
        notes.push(format!(
            "capture backend: {}",
            self.capture_backend.as_str()
        ));
        notes.push(
            "global hotkey: F2/Ctrl+N via global-hotkey when available; else `pinora capture` IPC"
                .into(),
        );
        match crate::image_sink::detect_system_clipboard_backend() {
            Some(b) => notes.push(format!("system clipboard: {b} (image/png)")),
            None => {
                #[cfg(unix)]
                notes.push("system clipboard: unavailable (install wl-clipboard or xclip)".into());
                #[cfg(not(unix))]
                notes.push("system clipboard: unavailable on this build; memory clipboard remains available".into());
            }
        }

        let global_hotkey_available = cfg!(target_os = "linux");
        if !global_hotkey_available {
            notes.push("global hotkey: unavailable on this build; use pinora capture IPC".into());
        }

        CapabilitySnapshot {
            capture_available: !matches!(self.capture_backend, CaptureBackendKind::Unavailable),
            global_hotkey_available,
            clipboard_image_available: crate::image_sink::detect_system_clipboard_backend()
                .is_some(),
            always_on_top_available: false,
            notes,
        }
    }
}

/// 测试用固定探测。
#[derive(Debug, Default, Clone, Copy)]
pub struct FakeCapabilityProbe;

impl CapabilityProbe for FakeCapabilityProbe {
    fn probe(&self) -> CapabilitySnapshot {
        let mut snapshot = RuntimeCapabilityProbe::new(
            CaptureBackendKind::Fake,
            Some("test probe: FakeCaptureProvider".into()),
        )
        .probe();
        snapshot.global_hotkey_available = true;
        snapshot.clipboard_image_available = true;
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_probe_mentions_backend() {
        let snap = RuntimeCapabilityProbe::new(CaptureBackendKind::Kde, Some("ok".into())).probe();
        assert!(snap.capture_available);
        assert!(snap.notes.iter().any(|n| n.contains("kde-spectacle")));
    }
}
