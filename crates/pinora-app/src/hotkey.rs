//! 热键/动作源抽象（本阶段无 OS 全局热键，仅内存注入）。

use std::sync::Mutex;

use pinora_core::{ActionId, KeyBinding, PinoraError};

/// 热键提供者：注册绑定并轮询触发的动作。
pub trait HotkeySource {
    fn register(&mut self, binding: KeyBinding) -> Result<(), PinoraError>;
    fn poll_actions(&mut self) -> Vec<ActionId>;
}

/// 可注入动作的假热键源（测试与开发演示）。
#[derive(Debug, Default)]
pub struct FakeHotkeySource {
    bindings: Vec<KeyBinding>,
    pending: Mutex<Vec<ActionId>>,
}

impl FakeHotkeySource {
    pub fn new() -> Self {
        Self::default()
    }

    /// 模拟用户按下某动作对应热键。
    pub fn inject(&self, action: ActionId) {
        if let Ok(mut q) = self.pending.lock() {
            q.push(action);
        }
    }

    pub fn bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }
}

impl HotkeySource for FakeHotkeySource {
    fn register(&mut self, binding: KeyBinding) -> Result<(), PinoraError> {
        self.bindings.push(binding);
        Ok(())
    }

    fn poll_actions(&mut self) -> Vec<ActionId> {
        self.pending
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_and_poll() {
        let mut src = FakeHotkeySource::new();
        src.register(KeyBinding::new(ActionId::Quit, "Ctrl+Q"))
            .unwrap();
        src.inject(ActionId::Quit);
        assert_eq!(src.poll_actions(), vec![ActionId::Quit]);
        assert!(src.poll_actions().is_empty());
    }
}
