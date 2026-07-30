//! Pinora 进程入口（仓库根 `src/main.rs`）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use pinora_app::{
    capture_region_interactive, AppRuntime, BootstrapOutcome, CaptureBackendKind,
    FakeHotkeySource, HotkeySource, LocalImageSink, OsSingleInstance, RuntimeCapabilityProbe,
    SelectedCaptureProvider,
};
use pinora_core::{
    ActionId, AppPhase, CaptureProvider, Command, DomainEventKind, KeyBinding, PixelPoint,
    PixelRect,
};

fn main() {
    let lock = match OsSingleInstance::default_paths() {
        Ok(lock) => lock,
        Err(err) => {
            eprintln!("pinora: single-instance setup failed: {err:?}");
            std::process::exit(1);
        }
    };

    let (capture, backend, note) = SelectedCaptureProvider::autodetect();
    println!(
        "pinora: capture backend = {}{}",
        backend.as_str(),
        note.as_ref()
            .map(|n| format!(" ({n})"))
            .unwrap_or_default()
    );

    let export_dir = lock.dir().join("export");
    let probe = RuntimeCapabilityProbe::new(backend, note);
    // default_capture_rect 仅作无显示环境回退；正常路径走 Overlay。
    let mut runtime = AppRuntime::new(lock, probe, capture, LocalImageSink::new()).with_defaults(
        PixelRect::new(100, 80, 320, 180),
        PixelPoint::new(120, 80),
        export_dir,
    );

    let mut hotkeys = FakeHotkeySource::new();
    let _ = hotkeys.register(KeyBinding::new(
        ActionId::CaptureRegionAndPin,
        "Ctrl+Shift+A",
    ));
    let _ = hotkeys.register(KeyBinding::new(ActionId::SaveLastCapture, "Ctrl+S"));
    let _ = hotkeys.register(KeyBinding::new(ActionId::CopyLastCapture, "Ctrl+C"));
    let _ = hotkeys.register(KeyBinding::new(ActionId::Quit, "Ctrl+Q"));

    match runtime.bootstrap() {
        Ok(BootstrapOutcome::Primary) => run_primary(&mut runtime, &mut hotkeys, backend),
        Ok(BootstrapOutcome::SecondaryForwarded) => {
            println!("pinora: another instance is running; forwarded Activate and exiting");
        }
        Err(err) => {
            eprintln!("pinora: bootstrap failed: {err}");
            std::process::exit(1);
        }
    }
}

fn run_primary(
    runtime: &mut AppRuntime<
        OsSingleInstance,
        RuntimeCapabilityProbe,
        SelectedCaptureProvider,
        LocalImageSink,
    >,
    hotkeys: &mut FakeHotkeySource,
    backend: CaptureBackendKind,
) {
    println!(
        "pinora: primary instance started (phase={:?})",
        runtime.state().phase
    );
    for note in &runtime.state().capabilities.notes {
        println!("pinora: capability note: {note}");
    }
    for binding in hotkeys.bindings() {
        println!(
            "pinora: hotkey registered (fake) {} => {}",
            binding.combo, binding.action
        );
    }

    println!("pinora: starting region selection overlay …");
    if let Err(err) = run_capture_region_action(runtime) {
        eprintln!("pinora: region capture failed: {err}");
        // 无图形环境时回退固定区域，避免完全不可用
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none()
        {
            eprintln!("pinora: no display; falling back to fixed rect demo");
            let _ = fallback_fixed_capture(runtime, backend);
        }
    }

    println!("pinora: GUI pin window not wired; process stays alive");
    println!("pinora: running — Ctrl+C to quit; second cargo run Activates this instance");

    let running = Arc::new(AtomicBool::new(true));
    let flag = Arc::clone(&running);
    if let Err(err) = ctrlc::set_handler(move || {
        flag.store(false, Ordering::SeqCst);
    }) {
        eprintln!("pinora: failed to install Ctrl+C handler: {err}");
        let _ = runtime.dispatch(Command::shutdown());
        std::process::exit(1);
    }

    while running.load(Ordering::SeqCst) {
        if let Ok(n) = runtime.poll_forwarded() {
            if n > 0 {
                println!(
                    "pinora: handled {n} forwarded command(s); activation_count={}",
                    runtime.state().activation_count
                );
            }
        }
        for action in hotkeys.poll_actions() {
            match action {
                ActionId::CaptureRegionAndPin => {
                    if let Err(err) = run_capture_region_action(runtime) {
                        eprintln!("pinora: action capture failed: {err}");
                    }
                }
                ActionId::Quit => {
                    running.store(false, Ordering::SeqCst);
                }
                other => match runtime.dispatch(Command::invoke_action(other)) {
                    Ok(_) => println!("pinora: action {other} applied"),
                    Err(err) => eprintln!("pinora: action {other} failed: {err}"),
                },
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    match runtime.dispatch(Command::shutdown()) {
        Ok(_) => println!(
            "pinora: shutdown complete (phase={:?}, pins={}, activations={})",
            runtime.state().phase,
            runtime.state().pin_count(),
            runtime.state().activation_count
        ),
        Err(err) => {
            eprintln!("pinora: shutdown failed: {err}");
            std::process::exit(1);
        }
    }

    if runtime.state().phase != AppPhase::Stopped {
        eprintln!("pinora: unexpected phase after shutdown");
        std::process::exit(1);
    }
}

fn run_capture_region_action(
    runtime: &mut AppRuntime<
        OsSingleInstance,
        RuntimeCapabilityProbe,
        SelectedCaptureProvider,
        LocalImageSink,
    >,
) -> Result<(), pinora_core::PinoraError> {
    let Some(result) = capture_region_interactive(runtime.capture_provider())? else {
        return Ok(());
    };

    let size = result.image.size();
    let pin = runtime.dispatch(Command::create_pin(result.image, result.pin_position))?;
    for event in &pin.events {
        if let DomainEventKind::PinCreated { pin_id, image_id } = event.event.kind {
            println!(
                "pinora: pin {pin_id} from {image_id} ({}x{}, pins={})",
                size.width,
                size.height,
                runtime.state().pin_count()
            );
        }
    }

    if let Ok(save) = runtime.dispatch(Command::invoke_action(ActionId::SaveLastCapture)) {
        for event in &save.events {
            if let DomainEventKind::ImageSaved { image_id, path } = &event.event.kind {
                println!("pinora: saved {image_id} -> {}", path.display());
            }
        }
    }
    if let Ok(copy) = runtime.dispatch(Command::invoke_action(ActionId::CopyLastCapture)) {
        for event in &copy.events {
            if let DomainEventKind::ImageCopied { image_id } = event.event.kind {
                println!(
                    "pinora: copied {image_id} to memory clipboard ({} bytes)",
                    runtime.sink().clipboard_byte_len().unwrap_or(0)
                );
            }
        }
    }
    Ok(())
}

fn fallback_fixed_capture(
    runtime: &mut AppRuntime<
        OsSingleInstance,
        RuntimeCapabilityProbe,
        SelectedCaptureProvider,
        LocalImageSink,
    >,
    backend: CaptureBackendKind,
) -> Result<(), pinora_core::PinoraError> {
    let _ = backend;
    let display = runtime
        .capture_provider()
        .displays()?
        .into_iter()
        .next()
        .ok_or_else(|| {
            pinora_core::PinoraError::new(
                pinora_core::ErrorCode::NotFound,
                "no display for fallback",
            )
        })?;
    let rect = PixelRect::new(
        display.bounds.origin.x,
        display.bounds.origin.y,
        320.min(display.bounds.size.width.max(1)),
        180.min(display.bounds.size.height.max(1)),
    );
    runtime.dispatch(Command::capture_and_pin(
        pinora_core::CaptureRequest::Region {
            display: display.id,
            rect,
        },
        PixelPoint::new(rect.origin.x + 24, rect.origin.y + 24),
    ))?;
    Ok(())
}
