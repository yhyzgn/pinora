//! Pinora 进程入口（仓库根 `src/main.rs`）。

use pinora_app::{
    run_desktop_shell, AppRuntime, BootstrapOutcome, LocalImageSink, OsSingleInstance,
    RuntimeCapabilityProbe, SelectedCaptureProvider,
};
use pinora_core::{PixelPoint, PixelRect};

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
    let mut runtime = AppRuntime::new(lock, probe, capture, LocalImageSink::new()).with_defaults(
        PixelRect::new(100, 80, 320, 180),
        PixelPoint::new(120, 80),
        export_dir,
    );

    match runtime.bootstrap() {
        Ok(BootstrapOutcome::Primary) => {
            println!(
                "pinora: primary started (phase={:?})",
                runtime.state().phase
            );
            for n in &runtime.state().capabilities.notes {
                println!("pinora: capability note: {n}");
            }
            println!("pinora: desktop shell — select region, then pin window");
            println!("pinora: pin: drag to move, scroll zoom, Esc close, F2/Ctrl+N new, Ctrl+Q quit");
            if let Err(err) = run_desktop_shell(runtime) {
                eprintln!("pinora: fatal: {err}");
                std::process::exit(1);
            }
        }
        Ok(BootstrapOutcome::SecondaryForwarded) => {
            println!("pinora: another instance is running; forwarded Activate and exiting");
        }
        Err(err) => {
            eprintln!("pinora: bootstrap failed: {err}");
            std::process::exit(1);
        }
    }
}
