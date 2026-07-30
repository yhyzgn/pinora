//! Pinora 进程入口（仓库根 `src/main.rs`）。

use pinora_app::{
    capture_region_interactive, run_pin_session, AppRuntime, BootstrapOutcome, CaptureBackendKind,
    LocalImageSink, OsSingleInstance, PinSessionEnd, PinView, RuntimeCapabilityProbe,
    SelectedCaptureProvider,
};
use pinora_core::{
    ActionId, AppPhase, CaptureProvider, Command, DomainEventKind, PixelPoint, PixelRect,
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
    let mut runtime = AppRuntime::new(lock, probe, capture, LocalImageSink::new()).with_defaults(
        PixelRect::new(100, 80, 320, 180),
        PixelPoint::new(120, 80),
        export_dir,
    );

    match runtime.bootstrap() {
        Ok(BootstrapOutcome::Primary) => {
            if let Err(err) = run_primary(&mut runtime, backend) {
                eprintln!("pinora: fatal: {err}");
                let _ = runtime.dispatch(Command::shutdown());
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

fn run_primary(
    runtime: &mut AppRuntime<
        OsSingleInstance,
        RuntimeCapabilityProbe,
        SelectedCaptureProvider,
        LocalImageSink,
    >,
    backend: CaptureBackendKind,
) -> Result<(), pinora_core::PinoraError> {
    println!(
        "pinora: primary instance started (phase={:?})",
        runtime.state().phase
    );
    for note in &runtime.state().capabilities.notes {
        println!("pinora: capability note: {note}");
    }

    let mut open_pins: Vec<PinView> = Vec::new();

    loop {
        // 单实例激活提示
        if let Ok(n) = runtime.poll_forwarded() {
            if n > 0 {
                println!(
                    "pinora: activated (count={})",
                    runtime.state().activation_count
                );
            }
        }

        println!("pinora: starting region selection …");
        match capture_one(runtime, backend) {
            Ok(Some(view)) => {
                open_pins.push(view);
            }
            Ok(None) => {
                println!("pinora: selection cancelled");
                if open_pins.is_empty() {
                    break;
                }
            }
            Err(err) => {
                eprintln!("pinora: capture failed: {err}");
                if open_pins.is_empty() {
                    break;
                }
            }
        }

        if open_pins.is_empty() {
            continue;
        }

        println!(
            "pinora: showing {} pin window(s) — drag/scroll/Esc; Ctrl+N 再截; Ctrl+Q 退出",
            open_pins.len()
        );
        let (end, closed_ids) = run_pin_session(open_pins.clone())?;
        for id in closed_ids {
            let _ = runtime.dispatch(Command::close_pin(id));
            open_pins.retain(|p| p.pin_id != id);
        }

        // 会话结束时若用户 NewCapture/Quit，窗口已全部销毁或仍保留
        match end {
            PinSessionEnd::NewCapture => {
                // 窗口已随事件循环结束销毁；未 Esc 关闭的贴图会在下次 run_pin_session 重建。
                continue;
            }
            PinSessionEnd::Quit => break,
            PinSessionEnd::AllClosed => {
                open_pins.clear();
                continue;
            }
        }
    }

    let _ = runtime.dispatch(Command::shutdown());
    println!(
        "pinora: shutdown complete (phase={:?}, pins={})",
        runtime.state().phase,
        runtime.state().pin_count()
    );
    let _ = AppPhase::Stopped;
    Ok(())
}

fn capture_one(
    runtime: &mut AppRuntime<
        OsSingleInstance,
        RuntimeCapabilityProbe,
        SelectedCaptureProvider,
        LocalImageSink,
    >,
    backend: CaptureBackendKind,
) -> Result<Option<PinView>, pinora_core::PinoraError> {
    let result = match capture_region_interactive(runtime.capture_provider()) {
        Ok(r) => r,
        Err(err) => {
            if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none()
            {
                eprintln!("pinora: no display; fixed-rect fallback");
                return fallback_fixed_pin(runtime, backend).map(Some);
            }
            return Err(err);
        }
    };

    let Some(result) = result else {
        return Ok(None);
    };

    let size = result.image.size();
    let image = result.image;
    let position = result.pin_position;
    let pin = runtime.dispatch(Command::create_pin(image.clone(), position))?;
    let pin_id = pin
        .events
        .iter()
        .find_map(|e| match e.event.kind {
            DomainEventKind::PinCreated { pin_id, .. } => Some(pin_id),
            _ => None,
        })
        .expect("PinCreated");

    println!(
        "pinora: pin {pin_id} created ({}x{})",
        size.width, size.height
    );

    if let Ok(save) = runtime.dispatch(Command::invoke_action(ActionId::SaveLastCapture)) {
        for event in &save.events {
            if let DomainEventKind::ImageSaved { image_id, path } = &event.event.kind {
                println!("pinora: saved {image_id} -> {}", path.display());
            }
        }
    }
    let _ = runtime.dispatch(Command::invoke_action(ActionId::CopyLastCapture));

    Ok(Some(PinView {
        pin_id,
        image,
        position,
        scale: 1.0,
    }))
}

fn fallback_fixed_pin(
    runtime: &mut AppRuntime<
        OsSingleInstance,
        RuntimeCapabilityProbe,
        SelectedCaptureProvider,
        LocalImageSink,
    >,
    _backend: CaptureBackendKind,
) -> Result<PinView, pinora_core::PinoraError> {
    let display = runtime
        .capture_provider()
        .displays()?
        .into_iter()
        .next()
        .ok_or_else(|| {
            pinora_core::PinoraError::new(pinora_core::ErrorCode::NotFound, "no display")
        })?;
    let rect = PixelRect::new(
        display.bounds.origin.x,
        display.bounds.origin.y,
        320.min(display.bounds.size.width.max(1)),
        180.min(display.bounds.size.height.max(1)),
    );
    let position = PixelPoint::new(rect.origin.x + 24, rect.origin.y + 24);
    let result = runtime.dispatch(Command::capture_and_pin(
        pinora_core::CaptureRequest::Region {
            display: display.id,
            rect,
        },
        position,
    ))?;
    let pin_id = result
        .events
        .iter()
        .find_map(|e| match e.event.kind {
            DomainEventKind::PinCreated { pin_id, .. } => Some(pin_id),
            _ => None,
        })
        .unwrap();
    let image_id = runtime.state().last_capture_id.unwrap();
    let image = runtime.state().image(image_id).unwrap().clone();
    Ok(PinView {
        pin_id,
        image,
        position,
        scale: 1.0,
    })
}
