//! Pinora 进程入口（仓库根 `src/main.rs`）。

use std::env;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use pinora_app::{
    AppRuntime, BootstrapOutcome, LocalImageSink, OsSingleInstance, RuntimeCapabilityProbe,
    SelectedCaptureProvider, SettingsLoad, SettingsStore, default_settings_path,
    ensure_user_desktop_entry, run_desktop_shell,
};
use pinora_core::{PixelPoint, PixelRect};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let action = parse_cli_action(&args);

    match SettingsStore::new(default_settings_path()).load() {
        SettingsLoad::Missing(_) => println!("pinora: settings unavailable; using defaults"),
        SettingsLoad::Loaded { repairs, .. } if repairs.is_empty() => {
            println!("pinora: settings loaded")
        }
        SettingsLoad::Loaded { .. } => println!("pinora: settings loaded with repaired values"),
        SettingsLoad::Invalid(_) => println!("pinora: settings invalid; using defaults"),
    }

    // 若已有实例：经 Unix socket 转发后退出（不抢锁）
    if let Some(frame) = action.ipc_frame() {
        if try_forward_to_running(frame) {
            println!("pinora: forwarded {:?} to running instance", action);
            return;
        }
        if matches!(action, CliAction::Quit) {
            println!("pinora: no running instance to quit");
            return;
        }
        if matches!(action, CliAction::Capture) {
            println!("pinora: no running instance; starting primary and capturing…");
        }
    }

    // 安装用户 desktop 入口（KDE 系统设置可绑定 pinora capture）
    if let Ok(exe) = env::current_exe() {
        match ensure_user_desktop_entry(&exe) {
            Ok(path) => println!("pinora: desktop entry → {}", path.display()),
            Err(e) => eprintln!("pinora: desktop entry skipped: {e}"),
        }
    }

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
        note.as_ref().map(|n| format!(" ({n})")).unwrap_or_default()
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
            println!(
                "pinora: keys: F2/Ctrl+N capture (global when available), Esc cancel/close, Ctrl+Q quit"
            );
            println!("pinora: cli: `pinora capture` / `pinora quit` (forwards to this instance)");
            if let Err(err) = run_desktop_shell(runtime) {
                eprintln!("pinora: fatal: {err}");
                std::process::exit(1);
            }
        }
        Ok(BootstrapOutcome::SecondaryForwarded) => {
            // 竞态：bootstrap 时已有实例；默认 Activate，若用户要 capture 再补一次
            if matches!(action, CliAction::Capture | CliAction::Default) {
                let _ = try_forward_to_running(b"CAPTURE\n");
            }
            println!("pinora: another instance is running; forwarded and exiting");
        }
        Err(err) => {
            eprintln!("pinora: bootstrap failed: {err}");
            std::process::exit(1);
        }
    }
}

/// 尝试连接已运行实例的 activate.sock 并写入 IPC 帧。
fn try_forward_to_running(frame: &[u8]) -> bool {
    let sock = runtime_sock_path();
    let Ok(mut stream) = UnixStream::connect(&sock) else {
        return false;
    };
    if stream.write_all(frame).is_err() {
        return false;
    }
    let _ = stream.flush();
    true
}

fn runtime_sock_path() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(xdg).join("pinora/activate.sock");
    }
    let user = env::var("USER").unwrap_or_else(|_| "user".into());
    std::env::temp_dir().join(format!("pinora-{user}/activate.sock"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliAction {
    Default,
    Capture,
    Quit,
}

impl CliAction {
    fn ipc_frame(self) -> Option<&'static [u8]> {
        match self {
            // 二次启动默认也触发截图（比仅 Activate 更有用）
            Self::Default | Self::Capture => Some(b"CAPTURE\n"),
            Self::Quit => Some(b"QUIT\n"),
        }
    }
}

fn parse_cli_action(args: &[String]) -> CliAction {
    if args.is_empty() {
        return CliAction::Default;
    }
    let a = args[0].as_str();
    match a {
        "capture" | "--capture" | "-c" => CliAction::Capture,
        "quit" | "--quit" | "-q" => CliAction::Quit,
        "help" | "--help" | "-h" => {
            print_help();
            std::process::exit(0);
        }
        _ => {
            eprintln!("pinora: unknown argument `{a}` (try --help)");
            std::process::exit(2);
        }
    }
}

fn print_help() {
    let exe = env::current_exe()
        .ok()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "pinora".into());
    println!(
        "\
pinora — screenshot / pin workbench

Usage:
  pinora              Start primary, or trigger capture if already running
  pinora capture      Trigger region capture on running instance
  pinora quit         Quit running instance

Global hotkeys (when OS registration succeeds):
  F2, Ctrl+N, Ctrl+Shift+S → capture

KDE System Settings (always works as backup):
  Shortcuts → Custom → Command:
    {exe} capture
"
    );
}
