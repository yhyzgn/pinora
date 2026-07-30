//! Pinora 进程入口（仓库根 `src/main.rs`）。
//!
//! Phase 0：尚无 GUI，但进程必须保持运行，直到收到退出信号再优雅关闭。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pinora_app::{
    AppRuntime, BootstrapOutcome, FakeCapabilityProbe, InMemorySingleInstance,
};
use pinora_core::{
    AppPhase, CaptureImage, CaptureMetadata, Command, DisplayId, DomainEventKind, ImageId,
    PixelPoint, PixelRect, PixelSize, RgbaBuffer,
};

fn main() {
    let mut runtime = AppRuntime::new(InMemorySingleInstance::new(), FakeCapabilityProbe);

    match runtime.bootstrap() {
        Ok(BootstrapOutcome::Primary) => {
            run_primary(&mut runtime);
        }
        Ok(BootstrapOutcome::SecondaryForwarded) => {
            // 内存单实例下，单独进程无法共享锁；此分支预留给后续跨进程实现。
            println!("pinora: secondary instance forwarded activate; exiting");
        }
        Err(err) => {
            eprintln!("pinora: bootstrap failed: {err}");
            std::process::exit(1);
        }
    }
}

fn run_primary(runtime: &mut AppRuntime<InMemorySingleInstance, FakeCapabilityProbe>) {
    println!(
        "pinora: primary instance started (phase={:?})",
        runtime.state().phase
    );
    for note in &runtime.state().capabilities.notes {
        println!("pinora: capability note: {note}");
    }

    if let Err(err) = seed_demo_pin(runtime) {
        eprintln!("pinora: demo pin failed: {err}");
        let _ = runtime.dispatch(Command::shutdown());
        std::process::exit(1);
    }

    println!("pinora: GUI/tray not wired yet; process stays alive in background mode");
    println!("pinora: running — press Ctrl+C to quit");

    if !wait_for_interrupt() {
        eprintln!("pinora: interrupt handler unavailable; exiting");
        let _ = runtime.dispatch(Command::shutdown());
        std::process::exit(1);
    }

    match runtime.dispatch(Command::shutdown()) {
        Ok(_) => println!(
            "pinora: shutdown complete (phase={:?}, pins={})",
            runtime.state().phase,
            runtime.state().pin_count()
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

/// 创建一张内存纯色演示截图并贴图，验证命令链路（非真实屏幕捕获）。
fn seed_demo_pin(
    runtime: &mut AppRuntime<InMemorySingleInstance, FakeCapabilityProbe>,
) -> Result<(), pinora_core::PinoraError> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let size = PixelSize::new(320, 180);
    let pixels = RgbaBuffer::solid(size, [0x2d, 0x6a, 0x4f, 0xff]);
    let image = CaptureImage::new(
        ImageId::new(),
        pixels,
        PixelRect::new(0, 0, size.width, size.height),
        CaptureMetadata::new(DisplayId::new("demo-display"), 1.0, now_ms),
    )
    .expect("demo capture image");

    let result = runtime.dispatch(Command::create_pin(image, PixelPoint::new(120, 80)))?;
    for event in &result.events {
        if let DomainEventKind::PinCreated { pin_id, image_id } = event.event.kind {
            println!(
                "pinora: demo pin created (pin={pin_id}, image={image_id}, size={}x{}, pins={})",
                size.width,
                size.height,
                runtime.state().pin_count()
            );
        }
    }
    Ok(())
}

/// 阻塞直到 Ctrl+C（SIGINT）或终止信号。成功安装处理器返回 `true`。
fn wait_for_interrupt() -> bool {
    let running = Arc::new(AtomicBool::new(true));
    let flag = Arc::clone(&running);

    if let Err(err) = ctrlc::set_handler(move || {
        flag.store(false, Ordering::SeqCst);
    }) {
        eprintln!("pinora: failed to install Ctrl+C handler: {err}");
        return false;
    }

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    true
}
