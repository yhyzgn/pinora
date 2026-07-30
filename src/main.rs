//! Pinora 进程入口（仓库根 `src/main.rs`）。
//!
//! Phase 0：fake 截图 + OS 单实例；尚无 GUI，进程保持运行直到 Ctrl+C。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use pinora_app::{
    AppRuntime, BootstrapOutcome, FakeCapabilityProbe, FakeCaptureProvider, OsSingleInstance,
};
use pinora_core::{
    AppPhase, CaptureRequest, Command, DomainEventKind, PixelPoint, PixelRect,
};

fn main() {
    let lock = match OsSingleInstance::default_paths() {
        Ok(lock) => lock,
        Err(err) => {
            eprintln!("pinora: single-instance setup failed: {err:?}");
            std::process::exit(1);
        }
    };

    let mut runtime = AppRuntime::new(lock, FakeCapabilityProbe, FakeCaptureProvider::new());

    match runtime.bootstrap() {
        Ok(BootstrapOutcome::Primary) => run_primary(&mut runtime),
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
    runtime: &mut AppRuntime<OsSingleInstance, FakeCapabilityProbe, FakeCaptureProvider>,
) {
    println!(
        "pinora: primary instance started (phase={:?})",
        runtime.state().phase
    );
    for note in &runtime.state().capabilities.notes {
        println!("pinora: capability note: {note}");
    }

    if let Err(err) = seed_demo_capture_pin(runtime) {
        eprintln!("pinora: demo capture/pin failed: {err}");
        let _ = runtime.dispatch(Command::shutdown());
        std::process::exit(1);
    }

    println!("pinora: GUI/tray not wired yet; process stays alive");
    println!(
        "pinora: running — Ctrl+C to quit (second `cargo run` will Activate this instance)"
    );

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
        match runtime.poll_forwarded() {
            Ok(n) if n > 0 => {
                println!(
                    "pinora: handled {n} forwarded command(s); activation_count={}",
                    runtime.state().activation_count
                );
            }
            Ok(_) => {}
            Err(err) => eprintln!("pinora: poll_forwarded error: {err}"),
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

/// 经 FakeCaptureProvider 捕获区域并创建贴图（非真实屏幕）。
fn seed_demo_capture_pin(
    runtime: &mut AppRuntime<OsSingleInstance, FakeCapabilityProbe, FakeCaptureProvider>,
) -> Result<(), pinora_core::PinoraError> {
    let display = runtime.capture_provider().primary_display_id();
    let capture = runtime.dispatch(Command::capture(CaptureRequest::Region {
        display,
        rect: PixelRect::new(100, 80, 320, 180),
    }))?;

    let image_id = capture
        .events
        .iter()
        .find_map(|e| match e.event.kind {
            DomainEventKind::CaptureCompleted { image_id, .. } => Some(image_id),
            _ => None,
        })
        .expect("CaptureCompleted event");

    let pin = runtime.dispatch(Command::create_pin_from_image(
        image_id,
        PixelPoint::new(120, 80),
    ))?;

    for event in &pin.events {
        if let DomainEventKind::PinCreated { pin_id, image_id } = event.event.kind {
            println!(
                "pinora: demo capture+pin (pin={pin_id}, image={image_id}, size=320x180, pins={})",
                runtime.state().pin_count()
            );
        }
    }
    Ok(())
}
