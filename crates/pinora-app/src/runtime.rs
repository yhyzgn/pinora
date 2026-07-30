use pinora_core::{
    AppPhase, AppState, Command, DomainEvent, DomainEventKind, ErrorCode, EventEnvelope,
    PinoraError,
};

use crate::platform::CapabilityProbe;
use crate::single_instance::{InstanceAcquisition, SingleInstance};

/// 启动结果：主实例运行，或二次启动应退出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapOutcome {
    /// 成为主实例并进入 Running。
    Primary,
    /// 已向主实例转发激活命令；当前进程应退出。
    SecondaryForwarded,
}

/// 单次命令分发结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchResult {
    pub events: Vec<EventEnvelope>,
}

/// 应用运行时：组装状态、单实例与命令处理。
pub struct AppRuntime<L, P> {
    state: AppState,
    lock: L,
    probe: P,
    events: Vec<EventEnvelope>,
}

impl<L, P> AppRuntime<L, P>
where
    L: SingleInstance,
    P: CapabilityProbe,
{
    pub fn new(lock: L, probe: P) -> Self {
        Self {
            state: AppState::new(),
            lock,
            probe,
            events: Vec::new(),
        }
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn events(&self) -> &[EventEnvelope] {
        &self.events
    }

    /// 尝试成为主实例；若已有实例则转发 Activate。
    pub fn bootstrap(&mut self) -> Result<BootstrapOutcome, PinoraError> {
        match self.lock.acquire()? {
            InstanceAcquisition::Acquired => {
                let cmd = Command::bootstrap();
                self.dispatch(cmd)?;
                Ok(BootstrapOutcome::Primary)
            }
            InstanceAcquisition::ExistingInstance => {
                let cmd = Command::activate();
                self.lock.forward(cmd)?;
                Ok(BootstrapOutcome::SecondaryForwarded)
            }
        }
    }

    /// 分发命令并记录事件。
    pub fn dispatch(&mut self, command: Command) -> Result<DispatchResult, PinoraError> {
        let correlation_id = command.correlation_id();
        let mut produced = Vec::new();

        match command {
            Command::Bootstrap { .. } => {
                if self.state.phase != AppPhase::Idle {
                    return Err(fail(
                        ErrorCode::AlreadyRunning,
                        "bootstrap requires Idle phase",
                    ));
                }
                self.state.capabilities = self.probe.probe();
                self.state.phase = AppPhase::Running;
                produced.push(EventEnvelope::now(
                    correlation_id,
                    DomainEvent {
                        kind: DomainEventKind::AppStarted,
                    },
                ));
            }
            Command::Activate { .. } => {
                if self.state.phase != AppPhase::Running {
                    return Err(fail(
                        ErrorCode::NotRunning,
                        "activate requires Running phase",
                    ));
                }
                self.state.activation_count = self.state.activation_count.saturating_add(1);
                produced.push(EventEnvelope::now(
                    correlation_id,
                    DomainEvent {
                        kind: DomainEventKind::AppActivated,
                    },
                ));
            }
            Command::Shutdown { .. } => {
                if self.state.phase != AppPhase::Running {
                    return Err(fail(
                        ErrorCode::NotRunning,
                        "shutdown requires Running phase",
                    ));
                }
                produced.push(EventEnvelope::now(
                    correlation_id,
                    DomainEvent {
                        kind: DomainEventKind::AppShuttingDown,
                    },
                ));
                self.lock.release()?;
                self.state.phase = AppPhase::Stopped;
                produced.push(EventEnvelope::now(
                    correlation_id,
                    DomainEvent {
                        kind: DomainEventKind::AppStopped,
                    },
                ));
            }
            Command::CreatePin {
                image, position, ..
            } => {
                self.require_running()?;
                let image_id = image.id;
                let pin_id = self.state.create_pin(image, position)?;
                produced.push(EventEnvelope::now(
                    correlation_id,
                    DomainEvent {
                        kind: DomainEventKind::PinCreated { pin_id, image_id },
                    },
                ));
            }
            Command::ClosePin { pin_id, .. } => {
                self.require_running()?;
                self.state.close_pin(pin_id)?;
                produced.push(EventEnvelope::now(
                    correlation_id,
                    DomainEvent {
                        kind: DomainEventKind::PinClosed { pin_id },
                    },
                ));
            }
            Command::SetPinTransform {
                pin_id, transform, ..
            } => {
                self.require_running()?;
                self.state.set_pin_transform(pin_id, transform)?;
                produced.push(EventEnvelope::now(
                    correlation_id,
                    DomainEvent {
                        kind: DomainEventKind::PinUpdated { pin_id },
                    },
                ));
            }
        }

        self.events.extend(produced.iter().cloned());
        Ok(DispatchResult { events: produced })
    }

    /// 将外部转发来的激活命令应用到主实例（测试与后续 IPC 使用）。
    pub fn apply_forwarded(&mut self, command: Command) -> Result<DispatchResult, PinoraError> {
        self.dispatch(command)
    }

    fn require_running(&self) -> Result<(), PinoraError> {
        if self.state.phase != AppPhase::Running {
            return Err(fail(
                ErrorCode::NotRunning,
                "command requires Running phase",
            ));
        }
        Ok(())
    }
}

fn fail(code: ErrorCode, message: impl Into<String>) -> PinoraError {
    PinoraError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::FakeCapabilityProbe;
    use crate::single_instance::InMemorySingleInstance;
    use pinora_core::{
        CaptureImage, CaptureMetadata, DisplayId, ImageId, PixelPoint, PixelRect, PixelSize,
        PinTransform, RgbaBuffer,
    };

    fn runtime() -> AppRuntime<InMemorySingleInstance, FakeCapabilityProbe> {
        AppRuntime::new(InMemorySingleInstance::new(), FakeCapabilityProbe)
    }

    fn sample_image() -> CaptureImage {
        let pixels = RgbaBuffer::solid(PixelSize::new(16, 9), [10, 20, 30, 255]);
        CaptureImage::new(
            ImageId::new(),
            pixels,
            PixelRect::new(0, 0, 16, 9),
            CaptureMetadata::new(DisplayId::new("display-0"), 1.0, 1),
        )
        .unwrap()
    }

    #[test]
    fn bootstrap_primary_enters_running() {
        let mut rt = runtime();
        let outcome = rt.bootstrap().expect("bootstrap");
        assert_eq!(outcome, BootstrapOutcome::Primary);
        assert_eq!(rt.state().phase, AppPhase::Running);
        assert!(
            rt.events()
                .iter()
                .any(|e| matches!(e.event.kind, DomainEventKind::AppStarted))
        );
        assert!(!rt.state().capabilities.capture_available);
    }

    #[test]
    fn secondary_bootstrap_forwards_activate() {
        let lock = InMemorySingleInstance::new();
        let mut primary = AppRuntime::new(lock.clone(), FakeCapabilityProbe);
        assert_eq!(primary.bootstrap().unwrap(), BootstrapOutcome::Primary);

        let mut secondary = AppRuntime::new(lock.clone(), FakeCapabilityProbe);
        let outcome = secondary.bootstrap().unwrap();
        assert_eq!(outcome, BootstrapOutcome::SecondaryForwarded);
        assert_eq!(secondary.state().phase, AppPhase::Idle);

        let forwarded = lock.forwarded_commands().unwrap();
        assert_eq!(forwarded.len(), 1);
        assert!(matches!(forwarded[0], Command::Activate { .. }));

        primary.apply_forwarded(forwarded[0].clone()).unwrap();
        assert_eq!(primary.state().activation_count, 1);
        assert!(
            primary
                .events()
                .iter()
                .any(|e| matches!(e.event.kind, DomainEventKind::AppActivated))
        );
    }

    #[test]
    fn shutdown_releases_lock_and_stops() {
        let lock = InMemorySingleInstance::new();
        let mut rt = AppRuntime::new(lock.clone(), FakeCapabilityProbe);
        rt.bootstrap().unwrap();
        let result = rt.dispatch(Command::shutdown()).unwrap();
        assert!(
            result
                .events
                .iter()
                .any(|e| matches!(e.event.kind, DomainEventKind::AppStopped))
        );
        assert_eq!(rt.state().phase, AppPhase::Stopped);

        let mut next = AppRuntime::new(lock, FakeCapabilityProbe);
        assert_eq!(next.bootstrap().unwrap(), BootstrapOutcome::Primary);
    }

    #[test]
    fn activate_while_idle_fails() {
        let mut rt = runtime();
        let err = rt.dispatch(Command::activate()).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotRunning);
    }

    #[test]
    fn double_bootstrap_on_same_runtime_fails() {
        let mut rt = runtime();
        rt.bootstrap().unwrap();
        let err = rt.dispatch(Command::bootstrap()).unwrap_err();
        assert_eq!(err.code, ErrorCode::AlreadyRunning);
    }

    #[test]
    fn events_carry_correlation_from_command() {
        let mut rt = runtime();
        let cmd = Command::bootstrap();
        let corr = cmd.correlation_id();
        let result = rt.dispatch(cmd).unwrap();
        assert!(result.events.iter().all(|e| e.correlation_id == corr));
    }

    #[test]
    fn create_and_close_pin_via_commands() {
        let mut rt = runtime();
        rt.bootstrap().unwrap();
        let image = sample_image();
        let image_id = image.id;
        let result = rt
            .dispatch(Command::create_pin(image, PixelPoint::new(40, 60)))
            .unwrap();
        let pin_id = match &result.events[0].event.kind {
            DomainEventKind::PinCreated { pin_id, image_id: iid } => {
                assert_eq!(*iid, image_id);
                *pin_id
            }
            other => panic!("unexpected event: {other:?}"),
        };
        assert_eq!(rt.state().pin_count(), 1);
        assert!(rt.state().image(image_id).is_some());

        rt.dispatch(Command::set_pin_transform(
            pin_id,
            PinTransform::default_at(PixelPoint::new(80, 90)),
        ))
        .unwrap();
        assert_eq!(
            rt.state().pin(pin_id).unwrap().transform.position,
            PixelPoint::new(80, 90)
        );

        rt.dispatch(Command::close_pin(pin_id)).unwrap();
        assert_eq!(rt.state().pin_count(), 0);
        assert!(rt.state().image(image_id).is_none());
        assert!(
            rt.events()
                .iter()
                .any(|e| matches!(e.event.kind, DomainEventKind::PinClosed { .. }))
        );
    }

    #[test]
    fn pin_commands_require_running() {
        let mut rt = runtime();
        let err = rt
            .dispatch(Command::create_pin(sample_image(), PixelPoint::new(0, 0)))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotRunning);
    }
}
