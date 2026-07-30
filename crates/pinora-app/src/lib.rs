//! Pinora 应用编排：生命周期、单实例与命令分发。

mod platform;
mod runtime;
mod single_instance;

pub use platform::{CapabilityProbe, FakeCapabilityProbe};
pub use runtime::{AppRuntime, BootstrapOutcome, DispatchResult};
pub use single_instance::{
    InMemorySingleInstance, InstanceAcquisition, SingleInstance, SingleInstanceError,
};
