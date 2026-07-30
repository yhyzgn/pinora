/// 应用生命周期阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppPhase {
    /// 尚未 bootstrap。
    Idle,
    /// 主实例正在运行。
    Running,
    /// 已关闭。
    Stopped,
}

/// 启动时探测到的平台能力摘要（业务逻辑不得直接读环境变量做分支）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilitySnapshot {
    pub capture_available: bool,
    pub global_hotkey_available: bool,
    pub clipboard_image_available: bool,
    pub always_on_top_available: bool,
    pub notes: Vec<String>,
}

/// 进程内应用状态（Phase 0 最小集）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub phase: AppPhase,
    pub capabilities: CapabilitySnapshot,
    pub activation_count: u64,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            phase: AppPhase::Idle,
            capabilities: CapabilitySnapshot::default(),
            activation_count: 0,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_idle() {
        let state = AppState::new();
        assert_eq!(state.phase, AppPhase::Idle);
        assert_eq!(state.activation_count, 0);
    }
}
