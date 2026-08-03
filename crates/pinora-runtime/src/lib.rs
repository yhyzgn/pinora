//! Pinora 应用工作流边界：命令分发、领域状态、单实例生命周期和事件发布。
//!
//! 本 crate 不创建窗口、不拥有 EventLoop，也不探测真实桌面环境；捕获与图像输出
//! 通过泛型端口注入，平台实例生命周期通过 `pinora-platform` 的单实例端口提供。

mod runtime;

pub use runtime::{AppRuntime, BootstrapOutcome, CapabilityProbe, DispatchResult};
