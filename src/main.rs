//! Pinora 进程入口（仓库根 `src/main.rs`）。
//!
//! Phase 0：尚无 GUI，但进程必须保持运行，直到收到退出信号再优雅关闭。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use pinora_app::{
    AppRuntime, BootstrapOutcome, FakeCapabilityProbe, InMemorySingleInstance,
};
use pinora_core::{AppPhase, Command};

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
    println!("pinora: GUI/tray not wired yet; process stays alive in background mode");
    println!("pinora: running — press Ctrl+C to quit");

    if !wait_for_interrupt() {
        eprintln!("pinora: interrupt handler unavailable; exiting");
        let _ = runtime.dispatch(Command::shutdown());
        std::process::exit(1);
    }

    match runtime.dispatch(Command::shutdown()) {
        Ok(_) => println!(
            "pinora: shutdown complete (phase={:?})",
            runtime.state().phase
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
