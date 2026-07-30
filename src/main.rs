//! Pinora 进程入口（仓库根 `src/main.rs`）。
//!
//! Phase 0：无 GUI 事件循环。演示 bootstrap → shutdown 的可测路径。

use pinora_app::{
    AppRuntime, BootstrapOutcome, FakeCapabilityProbe, InMemorySingleInstance,
};
use pinora_core::{AppPhase, Command};

fn main() {
    let mut runtime = AppRuntime::new(InMemorySingleInstance::new(), FakeCapabilityProbe);

    match runtime.bootstrap() {
        Ok(BootstrapOutcome::Primary) => {
            println!(
                "pinora: primary instance started (phase={:?})",
                runtime.state().phase
            );
            for note in &runtime.state().capabilities.notes {
                println!("pinora: capability note: {note}");
            }

            // Phase 0 尚无 GPUI 事件循环；完成一次干净关闭以验证退出路径。
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
