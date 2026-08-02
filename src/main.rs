//! Pinora 进程入口（仓库根 `src/main.rs`）。

use std::env;

use pinora_app::{
    AppRuntime, BootstrapOutcome, LocalImageSink, OsSingleInstance, RuntimeCapabilityProbe,
    SelectedCaptureProvider, SettingsLoad, SettingsStore, default_settings_path,
    ensure_user_desktop_entry, forward_ipc_frame, run_desktop_shell,
};
use pinora_core::{AppSettings, PixelPoint, PixelRect};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let action = parse_cli_action(&args);

    let settings = match SettingsStore::new(default_settings_path()).load() {
        SettingsLoad::Missing(settings) => {
            println!("pinora: settings unavailable; using defaults");
            settings
        }
        SettingsLoad::Loaded { settings, repairs } if repairs.is_empty() => {
            println!("pinora: settings loaded");
            settings
        }
        SettingsLoad::Loaded { settings, .. } => {
            println!("pinora: settings loaded with repaired values");
            settings
        }
        SettingsLoad::Invalid(_) => {
            println!("pinora: settings invalid; using defaults");
            AppSettings::default()
        }
    };

    // 若已有实例：经 Unix socket 转发后退出（不抢锁）
    if let Some(frame) = action.ipc_frame() {
        if forward_ipc_frame(frame) {
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
    let mut runtime = AppRuntime::new(lock, probe, capture, LocalImageSink::new())
        .with_defaults(
            PixelRect::new(100, 80, 320, 180),
            PixelPoint::new(120, 80),
            export_dir,
        )
        .with_settings(settings);

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
                "pinora: global keys: {}/Ctrl+N/Ctrl+Shift+S region, {} full display (when available), Esc cancel/close, Ctrl+Q quit",
                settings.region_hotkey, settings.full_display_hotkey
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
                let _ = forward_ipc_frame(b"CAPTURE\n");
            }
            println!("pinora: another instance is running; forwarded and exiting");
        }
        Err(err) => {
            eprintln!("pinora: bootstrap failed: {err}");
            std::process::exit(1);
        }
    }
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
        "version" | "--version" | "-V" => {
            let version = option_env!("PINORA_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
            println!("pinora {version}");
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
  pinora --version    Print the installed version

Global hotkeys (when OS registration succeeds; defaults shown):
  F2, Ctrl+N, Ctrl+Shift+S → region capture
  F3 → full-display capture
  Configure the primary region/full-display bindings in Settings.

KDE System Settings (always works as backup):
  Shortcuts → Custom → Command:
    {exe} capture
"
    );
}
