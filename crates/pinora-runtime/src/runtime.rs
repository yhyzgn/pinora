use std::path::PathBuf;

use pinora_core::{
    ActionId, AppPhase, AppSettings, AppState, CaptureProvider, CaptureRequest, Command,
    DomainEvent, DomainEventKind, ErrorCode, EventEnvelope, ImageSink, PinoraError, PixelPoint,
    PixelRect,
};

use pinora_platform::{InstanceAcquisition, SingleInstance};

/// 运行时启动时读取的平台能力端口。
pub trait CapabilityProbe {
    fn probe(&self) -> pinora_core::CapabilitySnapshot;
}

/// 启动结果：主实例运行，或二次启动应退出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapOutcome {
    Primary,
    SecondaryForwarded,
}

/// 单次命令分发结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchResult {
    pub events: Vec<EventEnvelope>,
}

/// 应用运行时。
pub struct AppRuntime<L, P, C, S> {
    state: AppState,
    lock: L,
    probe: P,
    capture: C,
    sink: S,
    /// InvokeAction(CaptureRegionAndPin) 使用的默认区域。
    default_capture_rect: PixelRect,
    default_pin_position: PixelPoint,
    default_export_dir: PathBuf,
    settings: AppSettings,
    events: Vec<EventEnvelope>,
}

impl<L, P, C, S> AppRuntime<L, P, C, S>
where
    L: SingleInstance,
    P: CapabilityProbe,
    C: CaptureProvider,
    S: ImageSink,
{
    pub fn new(lock: L, probe: P, capture: C, sink: S) -> Self {
        Self {
            state: AppState::new(),
            lock,
            probe,
            capture,
            sink,
            default_capture_rect: PixelRect::new(100, 80, 320, 180),
            default_pin_position: PixelPoint::new(120, 80),
            default_export_dir: std::env::temp_dir().join("pinora-export"),
            settings: AppSettings::default(),
            events: Vec::new(),
        }
    }

    pub fn with_defaults(
        mut self,
        capture_rect: PixelRect,
        pin_position: PixelPoint,
        export_dir: PathBuf,
    ) -> Self {
        self.default_capture_rect = capture_rect;
        self.default_pin_position = pin_position;
        self.default_export_dir = export_dir;
        self
    }

    /// 应用启动时注入经过 `SettingsStore` 校验的设置，并立即应用安全策略。
    pub fn with_settings(mut self, settings: AppSettings) -> Self {
        self.apply_settings(settings);
        self
    }

    /// 应用已通过 SettingsStore/领域校验的设置。
    ///
    /// 该方法只更新进程内策略；调用方应先成功持久化，再调用它，
    /// 从而避免磁盘写入失败时出现半应用状态。
    pub fn apply_settings(&mut self, settings: AppSettings) {
        let (settings, _) = settings.with_repaired_values();
        self.state.max_pins = settings.pin_limit as usize;
        self.settings = settings;
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    pub fn events(&self) -> &[EventEnvelope] {
        &self.events
    }

    pub fn capture_provider(&self) -> &C {
        &self.capture
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    pub fn export_dir(&self) -> &PathBuf {
        &self.default_export_dir
    }

    pub fn settings(&self) -> AppSettings {
        self.settings
    }

    pub fn bootstrap(&mut self) -> Result<BootstrapOutcome, PinoraError> {
        match self.lock.acquire()? {
            InstanceAcquisition::Acquired => {
                self.dispatch(Command::bootstrap())?;
                Ok(BootstrapOutcome::Primary)
            }
            InstanceAcquisition::ExistingInstance => {
                self.lock.forward(Command::activate())?;
                Ok(BootstrapOutcome::SecondaryForwarded)
            }
        }
    }

    pub fn poll_forwarded(&mut self) -> Result<usize, PinoraError> {
        let commands = self.lock.poll_forwarded()?;
        let n = commands.len();
        for command in commands {
            self.dispatch(command)?;
        }
        Ok(n)
    }

    /// 取出转发命令但不分发，由桌面 shell 解释 Capture/Quit 等。
    pub fn take_forwarded(&mut self) -> Result<Vec<Command>, PinoraError> {
        self.lock.poll_forwarded().map_err(Into::into)
    }

    pub fn lock(&self) -> &L {
        &self.lock
    }

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
                produced.push(event(correlation_id, DomainEventKind::AppStarted));
            }
            Command::Activate { .. } => {
                if self.state.phase != AppPhase::Running {
                    return Err(fail(
                        ErrorCode::NotRunning,
                        "activate requires Running phase",
                    ));
                }
                self.state.activation_count = self.state.activation_count.saturating_add(1);
                produced.push(event(correlation_id, DomainEventKind::AppActivated));
            }
            Command::Shutdown { .. } => {
                if self.state.phase != AppPhase::Running {
                    return Err(fail(
                        ErrorCode::NotRunning,
                        "shutdown requires Running phase",
                    ));
                }
                produced.push(event(correlation_id, DomainEventKind::AppShuttingDown));
                self.lock.release()?;
                self.state.phase = AppPhase::Stopped;
                produced.push(event(correlation_id, DomainEventKind::AppStopped));
            }
            Command::Capture { request, .. } => {
                self.require_running()?;
                produced.extend(self.do_capture(correlation_id, request)?);
            }
            Command::CaptureAndPin {
                request, position, ..
            } => {
                self.require_running()?;
                let cap_events = self.do_capture(correlation_id, request)?;
                let image_id = cap_events
                    .iter()
                    .find_map(|e| match e.event.kind {
                        DomainEventKind::CaptureCompleted { image_id, .. } => Some(image_id),
                        _ => None,
                    })
                    .expect("capture yields image");
                produced.extend(cap_events);
                let pin_id = self.state.create_pin_from_image(image_id, position)?;
                produced.push(event(
                    correlation_id,
                    DomainEventKind::PinCreated { pin_id, image_id },
                ));
            }
            Command::CreatePin {
                image, position, ..
            } => {
                self.require_running()?;
                let image_id = image.id;
                let pin_id = self.state.create_pin(image, position)?;
                produced.push(event(
                    correlation_id,
                    DomainEventKind::PinCreated { pin_id, image_id },
                ));
            }
            Command::CreatePinFromImage {
                image_id, position, ..
            } => {
                self.require_running()?;
                let pin_id = self.state.create_pin_from_image(image_id, position)?;
                produced.push(event(
                    correlation_id,
                    DomainEventKind::PinCreated { pin_id, image_id },
                ));
            }
            Command::ClosePin { pin_id, .. } => {
                self.require_running()?;
                self.state.close_pin(pin_id)?;
                produced.push(event(correlation_id, DomainEventKind::PinClosed { pin_id }));
            }
            Command::SetPinTransform {
                pin_id, transform, ..
            } => {
                self.require_running()?;
                self.state.set_pin_transform(pin_id, transform)?;
                produced.push(event(
                    correlation_id,
                    DomainEventKind::PinUpdated { pin_id },
                ));
            }
            Command::ReplacePinImage { pin_id, image, .. } => {
                self.require_running()?;
                self.state.replace_pin_image(pin_id, image)?;
                produced.push(event(
                    correlation_id,
                    DomainEventKind::PinUpdated { pin_id },
                ));
            }
            Command::SetPinLocked { pin_id, locked, .. } => {
                self.require_running()?;
                self.state.set_pin_locked(pin_id, locked)?;
                produced.push(event(
                    correlation_id,
                    DomainEventKind::PinUpdated { pin_id },
                ));
            }
            Command::SetPinAlwaysOnTop {
                pin_id,
                always_on_top,
                ..
            } => {
                self.require_running()?;
                self.state.set_pin_always_on_top(pin_id, always_on_top)?;
                produced.push(event(
                    correlation_id,
                    DomainEventKind::PinUpdated { pin_id },
                ));
            }
            Command::SavePng { image_id, path, .. } => {
                self.require_running()?;
                let image = self.state.image(image_id).cloned().ok_or_else(|| {
                    fail(ErrorCode::NotFound, format!("image not found: {image_id}"))
                })?;
                self.sink.save_png(&image, &path)?;
                produced.push(event(
                    correlation_id,
                    DomainEventKind::ImageSaved { image_id, path },
                ));
            }
            Command::CopyImage { image_id, .. } => {
                self.require_running()?;
                let image = self.state.image(image_id).cloned().ok_or_else(|| {
                    fail(ErrorCode::NotFound, format!("image not found: {image_id}"))
                })?;
                self.sink.copy_image(&image)?;
                produced.push(event(
                    correlation_id,
                    DomainEventKind::ImageCopied { image_id },
                ));
            }
            Command::InvokeAction { action, .. } => {
                self.require_running()?;
                let nested = self.expand_action(action)?;
                let nested_result = self.dispatch(nested)?;
                return Ok(nested_result);
            }
        }

        self.events.extend(produced.iter().cloned());
        Ok(DispatchResult { events: produced })
    }

    pub fn apply_forwarded(&mut self, command: Command) -> Result<DispatchResult, PinoraError> {
        self.dispatch(command)
    }

    fn expand_action(&self, action: ActionId) -> Result<Command, PinoraError> {
        match action {
            ActionId::CaptureRegionAndPin => {
                let displays = self.capture.displays()?;
                let display = displays
                    .first()
                    .map(|d| d.id.clone())
                    .ok_or_else(|| fail(ErrorCode::NotFound, "no display for capture"))?;
                Ok(Command::capture_and_pin(
                    CaptureRequest::Region {
                        display,
                        rect: self.default_capture_rect,
                    },
                    self.default_pin_position,
                ))
            }
            ActionId::CaptureFullDisplay => {
                let displays = self.capture.displays()?;
                let display = displays
                    .first()
                    .map(|d| d.id.clone())
                    .ok_or_else(|| fail(ErrorCode::NotFound, "no display for capture"))?;
                Ok(Command::capture_and_pin(
                    CaptureRequest::FullDisplay { display },
                    self.default_pin_position,
                ))
            }
            ActionId::SaveLastCapture => {
                let image_id = self
                    .state
                    .last_capture_id
                    .ok_or_else(|| fail(ErrorCode::NotFound, "no last capture to save"))?;
                let path = self.default_export_dir.join(format!("{image_id}.png"));
                Ok(Command::save_png(image_id, path))
            }
            ActionId::CopyLastCapture => {
                let image_id = self
                    .state
                    .last_capture_id
                    .ok_or_else(|| fail(ErrorCode::NotFound, "no last capture to copy"))?;
                Ok(Command::copy_image(image_id))
            }
            ActionId::Quit => Ok(Command::shutdown()),
        }
    }

    fn do_capture(
        &mut self,
        correlation_id: pinora_core::CorrelationId,
        request: CaptureRequest,
    ) -> Result<Vec<EventEnvelope>, PinoraError> {
        let image = self.capture.capture(request)?;
        let image_id = image.id;
        let size = image.size();
        self.state.retain_image(image);
        Ok(vec![event(
            correlation_id,
            DomainEventKind::CaptureCompleted { image_id, size },
        )])
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

fn event(correlation_id: pinora_core::CorrelationId, kind: DomainEventKind) -> EventEnvelope {
    EventEnvelope::now(correlation_id, DomainEvent { kind })
}

fn fail(code: ErrorCode, message: impl Into<String>) -> PinoraError {
    PinoraError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_capture::FakeCaptureProvider;
    use pinora_core::{PinTransform, PixelSize};
    use pinora_export::LocalImageSink;
    use pinora_platform::InMemorySingleInstance;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug, Default, Clone, Copy)]
    struct FakeCapabilityProbe;

    impl CapabilityProbe for FakeCapabilityProbe {
        fn probe(&self) -> pinora_core::CapabilitySnapshot {
            pinora_core::CapabilitySnapshot {
                capture_available: true,
                global_hotkey_available: true,
                clipboard_image_available: true,
                always_on_top_available: false,
                notes: vec!["test probe: fake".into()],
            }
        }
    }

    type TestRt = AppRuntime<
        InMemorySingleInstance,
        FakeCapabilityProbe,
        FakeCaptureProvider,
        LocalImageSink,
    >;

    fn runtime() -> TestRt {
        AppRuntime::new(
            InMemorySingleInstance::new(),
            FakeCapabilityProbe,
            FakeCaptureProvider::new(),
            LocalImageSink::new(),
        )
    }

    #[test]
    fn settings_apply_pin_limit_to_runtime_state() {
        let settings = AppSettings {
            pin_limit: 2,
            default_pin_opacity_percent: 72,
            ..AppSettings::default()
        };
        let rt = runtime().with_settings(settings);
        assert_eq!(rt.state().max_pins, 2);
        assert_eq!(rt.settings().default_pin_opacity_percent, 72);
    }

    #[test]
    fn settings_apply_preserves_ocr_confidence_threshold() {
        let settings = AppSettings {
            ocr_confidence_threshold: 85,
            ..AppSettings::default()
        };

        let rt = runtime().with_settings(settings);

        assert_eq!(rt.settings().ocr_confidence_threshold, 85);
    }

    #[test]
    fn invalid_numeric_settings_are_repaired_before_runtime_application() {
        let settings = AppSettings {
            pin_limit: 0,
            default_pin_opacity_percent: 14,
            ..AppSettings::default()
        };
        let rt = runtime().with_settings(settings);
        assert_eq!(rt.state().max_pins, 10);
        assert_eq!(rt.settings().default_pin_opacity_percent, 100);
    }

    #[test]
    fn bootstrap_primary_enters_running() {
        let mut rt = runtime();
        assert_eq!(rt.bootstrap().unwrap(), BootstrapOutcome::Primary);
        assert_eq!(rt.state().phase, AppPhase::Running);
        assert!(rt.state().capabilities.capture_available);
        assert!(rt.state().capabilities.clipboard_image_available);
    }

    #[test]
    fn secondary_bootstrap_forwards_activate() {
        let lock = InMemorySingleInstance::new();
        let mut primary = AppRuntime::new(
            lock.clone(),
            FakeCapabilityProbe,
            FakeCaptureProvider::new(),
            LocalImageSink::new(),
        );
        primary.bootstrap().unwrap();
        let mut secondary = AppRuntime::new(
            lock,
            FakeCapabilityProbe,
            FakeCaptureProvider::new(),
            LocalImageSink::new(),
        );
        assert_eq!(
            secondary.bootstrap().unwrap(),
            BootstrapOutcome::SecondaryForwarded
        );
        assert_eq!(primary.poll_forwarded().unwrap(), 1);
        assert_eq!(primary.state().activation_count, 1);
    }

    #[test]
    fn capture_and_pin_then_export() {
        let mut rt = runtime();
        rt.bootstrap().unwrap();
        let display = rt.capture_provider().primary_display_id();
        let result = rt
            .dispatch(Command::capture_and_pin(
                CaptureRequest::Region {
                    display,
                    rect: PixelRect::new(0, 0, 40, 30),
                },
                PixelPoint::new(1, 2),
            ))
            .unwrap();
        assert!(
            result
                .events
                .iter()
                .any(|e| matches!(e.event.kind, DomainEventKind::CaptureCompleted { .. }))
        );
        assert!(
            result
                .events
                .iter()
                .any(|e| matches!(e.event.kind, DomainEventKind::PinCreated { .. }))
        );
        assert_eq!(rt.state().pin_count(), 1);
        let image_id = rt.state().last_capture_id.unwrap();

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("pinora-rt-{nanos}.png"));
        rt.dispatch(Command::save_png(image_id, path.clone()))
            .unwrap();
        assert!(path.is_file());
        let _ = std::fs::remove_file(&path);

        if let Err(error) = rt.dispatch(Command::copy_image(image_id)) {
            assert_eq!(error.code, ErrorCode::ClipboardFailed);
        }
        assert_eq!(rt.sink().clipboard_image_id(), Some(image_id));
    }

    #[test]
    fn invoke_action_capture_region_and_pin() {
        let mut rt = runtime();
        rt.bootstrap().unwrap();
        rt.dispatch(Command::invoke_action(ActionId::CaptureRegionAndPin))
            .unwrap();
        assert_eq!(rt.state().pin_count(), 1);
        assert!(rt.state().last_capture_id.is_some());
    }

    #[test]
    fn invoke_action_capture_full_display_and_pin() {
        let mut rt = runtime();
        rt.bootstrap().unwrap();
        let display = rt.capture_provider().displays().unwrap().remove(0);

        rt.dispatch(Command::invoke_action(ActionId::CaptureFullDisplay))
            .unwrap();

        let image_id = rt.state().last_capture_id.unwrap();
        assert_eq!(
            rt.state().image(image_id).unwrap().source_rect,
            display.bounds
        );
        assert_eq!(rt.state().pin_count(), 1);
    }

    #[test]
    fn invoke_save_and_copy_last() {
        let mut rt = runtime().with_defaults(
            PixelRect::new(0, 0, 16, 16),
            PixelPoint::new(0, 0),
            std::env::temp_dir().join("pinora-action-export"),
        );
        rt.bootstrap().unwrap();
        rt.dispatch(Command::invoke_action(ActionId::CaptureRegionAndPin))
            .unwrap();
        let image_id = rt.state().last_capture_id.unwrap();
        rt.dispatch(Command::invoke_action(ActionId::SaveLastCapture))
            .unwrap();
        let path = rt.export_dir().join(format!("{image_id}.png"));
        assert!(path.is_file());
        let _ = std::fs::remove_file(&path);

        if let Err(error) = rt.dispatch(Command::invoke_action(ActionId::CopyLastCapture)) {
            assert_eq!(error.code, ErrorCode::ClipboardFailed);
        }
        assert_eq!(rt.sink().clipboard_image_id(), Some(image_id));
    }

    #[test]
    fn create_and_close_pin_via_commands() {
        let mut rt = runtime();
        rt.bootstrap().unwrap();
        let display = rt.capture_provider().primary_display_id();
        let image = rt
            .capture
            .capture(CaptureRequest::Region {
                display,
                rect: PixelRect::new(0, 0, 16, 9),
            })
            .unwrap();
        let result = rt
            .dispatch(Command::create_pin(image, PixelPoint::new(40, 60)))
            .unwrap();
        let pin_id = match &result.events[0].event.kind {
            DomainEventKind::PinCreated { pin_id, .. } => *pin_id,
            other => panic!("{other:?}"),
        };
        rt.dispatch(Command::set_pin_transform(
            pin_id,
            PinTransform::default_at(PixelPoint::new(80, 90)),
        ))
        .unwrap();
        rt.dispatch(Command::close_pin(pin_id)).unwrap();
        assert_eq!(rt.state().pin_count(), 0);
    }

    #[test]
    fn pin_edit_commands_preserve_identity_and_publish_updates() {
        let mut rt = runtime();
        rt.bootstrap().unwrap();
        let display = rt.capture_provider().primary_display_id();
        let original = rt
            .capture
            .capture(CaptureRequest::Region {
                display: display.clone(),
                rect: PixelRect::new(0, 0, 16, 9),
            })
            .unwrap();
        let original_id = original.id;
        let created = rt
            .dispatch(Command::create_pin(original, PixelPoint::new(40, 60)))
            .unwrap();
        let pin_id = match created.events[0].event.kind {
            DomainEventKind::PinCreated { pin_id, .. } => pin_id,
            ref other => panic!("{other:?}"),
        };
        let replacement = rt
            .capture
            .capture(CaptureRequest::Region {
                display,
                rect: PixelRect::new(1, 1, 8, 6),
            })
            .unwrap();
        let replacement_id = replacement.id;

        for command in [
            Command::set_pin_locked(pin_id, true),
            Command::set_pin_always_on_top(pin_id, false),
            Command::replace_pin_image(pin_id, replacement),
        ] {
            let result = rt.dispatch(command).unwrap();
            assert!(matches!(
                result.events.as_slice(),
                [event] if matches!(event.event.kind, DomainEventKind::PinUpdated { pin_id: updated } if updated == pin_id)
            ));
        }

        let pin = rt.state().pin(pin_id).unwrap();
        assert_eq!(pin.id, pin_id);
        assert_eq!(pin.image_id, replacement_id);
        assert!(pin.locked);
        assert!(!pin.always_on_top);
        assert!(rt.state().image(original_id).is_none());
        assert!(rt.state().image(replacement_id).is_some());
    }

    #[test]
    fn pin_commands_require_running() {
        let mut rt = runtime();
        let err = rt
            .dispatch(Command::invoke_action(ActionId::CaptureRegionAndPin))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotRunning);
    }

    #[test]
    fn shutdown_releases_lock() {
        let lock = InMemorySingleInstance::new();
        let mut rt = AppRuntime::new(
            lock.clone(),
            FakeCapabilityProbe,
            FakeCaptureProvider::new(),
            LocalImageSink::new(),
        );
        rt.bootstrap().unwrap();
        rt.dispatch(Command::shutdown()).unwrap();
        let mut next = AppRuntime::new(
            lock,
            FakeCapabilityProbe,
            FakeCaptureProvider::new(),
            LocalImageSink::new(),
        );
        assert_eq!(next.bootstrap().unwrap(), BootstrapOutcome::Primary);
    }

    #[test]
    fn capture_size_event() {
        let mut rt = runtime();
        rt.bootstrap().unwrap();
        let display = rt.capture_provider().primary_display_id();
        let result = rt
            .dispatch(Command::capture(CaptureRequest::Region {
                display,
                rect: PixelRect::new(0, 0, 64, 48),
            }))
            .unwrap();
        match &result.events[0].event.kind {
            DomainEventKind::CaptureCompleted { size, .. } => {
                assert_eq!(*size, PixelSize::new(64, 48));
            }
            other => panic!("{other:?}"),
        }
    }
}
