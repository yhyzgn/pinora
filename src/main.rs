//! Pinora 进程入口（仓库根 `src/main.rs`）。
//!
//! Phase 0+：fake 截图、OS 单实例、PNG 导出与内存剪贴板；无 GUI。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use pinora_app::{
    AppRuntime, BootstrapOutcome, FakeCapabilityProbe, FakeCaptureProvider, FakeHotkeySource,
    HotkeySource, LocalImageSink, OsSingleInstance,
};
use pinora_core::{
    ActionId, AppPhase, Command, DomainEventKind, KeyBinding, PixelPoint, PixelRect,
};

fn main() {
    let lock = match OsSingleInstance::default_paths() {
        Ok(lock) => lock,
        Err(err) => {
            eprintln!("pinora: single-instance setup failed: {err:?}");
            std::process::exit(1);
        }
    };

    let export_dir = lock.dir().join("export");
    let mut runtime = AppRuntime::new(
        lock,
        FakeCapabilityProbe,
        FakeCaptureProvider::new(),
        LocalImageSink::new(),
    )
    .with_defaults(
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
        Ok(BootstrapOutcome::Primary) => run_primary(&mut runtime, &mut hotkeys),
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
        FakeCapabilityProbe,
        FakeCaptureProvider,
        LocalImageSink,
    >,
    hotkeys: &mut FakeHotkeySource,
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

    if let Err(err) = seed_demo_workflow(runtime) {
        eprintln!("pinora: demo workflow failed: {err}");
        let _ = runtime.dispatch(Command::shutdown());
        std::process::exit(1);
    }

    println!("pinora: GUI/global hotkeys not wired; process stays alive");
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
            match runtime.dispatch(Command::invoke_action(action)) {
                Ok(_) => println!("pinora: action {action} applied"),
                Err(err) => eprintln!("pinora: action {action} failed: {err}"),
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

fn seed_demo_workflow(
    runtime: &mut AppRuntime<
        OsSingleInstance,
        FakeCapabilityProbe,
        FakeCaptureProvider,
        LocalImageSink,
    >,
) -> Result<(), pinora_core::PinoraError> {
    let result = runtime.dispatch(Command::invoke_action(ActionId::CaptureRegionAndPin))?;
    for event in &result.events {
        match &event.event.kind {
            DomainEventKind::CaptureCompleted { image_id, size } => {
                println!(
                    "pinora: captured {image_id} ({}x{})",
                    size.width, size.height
                );
            }
            DomainEventKind::PinCreated { pin_id, image_id } => {
                println!(
                    "pinora: pin {pin_id} from {image_id} (pins={})",
                    runtime.state().pin_count()
                );
            }
            _ => {}
        }
    }

    let save = runtime.dispatch(Command::invoke_action(ActionId::SaveLastCapture))?;
    for event in &save.events {
        if let DomainEventKind::ImageSaved { image_id, path } = &event.event.kind {
            println!("pinora: saved {image_id} -> {}", path.display());
        }
    }

    let copy = runtime.dispatch(Command::invoke_action(ActionId::CopyLastCapture))?;
    for event in &copy.events {
        if let DomainEventKind::ImageCopied { image_id } = event.event.kind {
            println!(
                "pinora: copied {image_id} to memory clipboard ({} bytes)",
                runtime.sink().clipboard_byte_len().unwrap_or(0)
            );
        }
    }
    Ok(())
}
