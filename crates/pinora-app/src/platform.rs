use pinora_core::CapabilitySnapshot;

use crate::capture_select::CaptureBackendKind;

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
        notes.push("clipboard=LocalImageSink memory only".into());
        notes.push("global hotkey not wired (FakeHotkeySource inject only)".into());

        CapabilitySnapshot {
            capture_available: true,
            global_hotkey_available: false,
            clipboard_image_available: true,
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
        RuntimeCapabilityProbe::new(
            CaptureBackendKind::Fake,
            Some("test probe: FakeCaptureProvider".into()),
        )
        .probe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_probe_mentions_backend() {
        let snap = RuntimeCapabilityProbe::new(CaptureBackendKind::Xcap, Some("ok".into())).probe();
        assert!(snap.capture_available);
        assert!(snap.notes.iter().any(|n| n.contains("xcap")));
    }
}
