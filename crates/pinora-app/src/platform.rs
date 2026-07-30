use pinora_core::CapabilitySnapshot;

/// 平台能力探测。
pub trait CapabilityProbe {
    fn probe(&self) -> CapabilitySnapshot;
}

/// 开发期假探测：fake 捕获与内存剪贴板可用。
#[derive(Debug, Default, Clone, Copy)]
pub struct FakeCapabilityProbe;

impl CapabilityProbe for FakeCapabilityProbe {
    fn probe(&self) -> CapabilitySnapshot {
        CapabilitySnapshot {
            capture_available: true,
            global_hotkey_available: false,
            clipboard_image_available: true,
            always_on_top_available: false,
            notes: vec![
                "fake probe: CaptureProvider=FakeCaptureProvider (not real screen)".into(),
                "fake probe: clipboard=LocalImageSink memory only".into(),
                "fake probe: global hotkey not wired (FakeHotkeySource inject only)".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_probe_marks_capture_and_clipboard() {
        let snap = FakeCapabilityProbe.probe();
        assert!(snap.capture_available);
        assert!(snap.clipboard_image_available);
        assert!(!snap.global_hotkey_available);
    }
}
