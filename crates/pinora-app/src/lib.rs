//! Pinora 应用编排：生命周期、单实例与命令分发。

mod capture_fake;
mod hotkey;
mod image_sink;
mod os_instance;
mod platform;
mod runtime;
mod single_instance;

pub use capture_fake::FakeCaptureProvider;
pub use hotkey::{FakeHotkeySource, HotkeySource};
pub use image_sink::LocalImageSink;
pub use os_instance::OsSingleInstance;
pub use platform::{CapabilityProbe, FakeCapabilityProbe};
pub use runtime::{AppRuntime, BootstrapOutcome, DispatchResult};
pub use single_instance::{
    InMemorySingleInstance, InstanceAcquisition, SingleInstance, SingleInstanceError,
};
