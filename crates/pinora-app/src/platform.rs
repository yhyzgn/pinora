use pinora_core::CapabilitySnapshot;

/// 平台能力探测。
pub trait CapabilityProbe {
    fn probe(&self) -> CapabilitySnapshot;
}

/// 假实现：标记核心能力不可用，附带说明（Phase 0 无真实平台适配）。
#[derive(Debug, Default, Clone, Copy)]
pub struct FakeCapabilityProbe;

impl CapabilityProbe for FakeCapabilityProbe {
    fn probe(&self) -> CapabilitySnapshot {
        CapabilitySnapshot {
            capture_available: false,
            global_hotkey_available: false,
            clipboard_image_available: false,
            always_on_top_available: false,
            notes: vec![
                "fake probe: desktop capture not wired".into(),
                "fake probe: global hotkey not wired".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_probe_reports_unavailable_capabilities() {
        let snap = FakeCapabilityProbe.probe();
        assert!(!snap.capture_available);
        assert!(!snap.global_hotkey_available);
        assert_eq!(snap.notes.len(), 2);
    }
}
