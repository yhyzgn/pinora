use pinora_core::CapabilitySnapshot;

/// 平台能力探测。
pub trait CapabilityProbe {
    fn probe(&self) -> CapabilitySnapshot;
}

/// 开发期假探测：标记 fake 捕获可用，真实热键/剪贴板未接线。
#[derive(Debug, Default, Clone, Copy)]
pub struct FakeCapabilityProbe;

impl CapabilityProbe for FakeCapabilityProbe {
    fn probe(&self) -> CapabilitySnapshot {
        CapabilitySnapshot {
            capture_available: true,
            global_hotkey_available: false,
            clipboard_image_available: false,
            always_on_top_available: false,
            notes: vec![
                "fake probe: CaptureProvider=FakeCaptureProvider (not real screen)".into(),
                "fake probe: global hotkey not wired".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_probe_marks_capture_available() {
        let snap = FakeCapabilityProbe.probe();
        assert!(snap.capture_available);
        assert!(!snap.global_hotkey_available);
    }
}
