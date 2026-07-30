//! 统一桌面事件循环：选区 Overlay + 贴图窗口（同一 EventLoop，适配 Wayland）。

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use pinora_core::{
    ActionId, CaptureImage, CaptureProvider, CaptureRequest, Command, DomainEventKind, ErrorCode,
    ImageSink, PinId, PinoraError, PixelPoint, PixelRect, SelectionSession,
};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{CursorIcon, Fullscreen, Window, WindowId, WindowLevel};

use crate::pin_window::scaled_window_size;
use crate::platform::CapabilityProbe;
use crate::runtime::AppRuntime;
use crate::single_instance::SingleInstance;

const MIN_FRAME_INTERVAL: Duration = Duration::from_micros(16_666);

/// 运行统一桌面 shell（阻塞直到退出）。
pub fn run_desktop_shell<L, P, C, S>(
    runtime: AppRuntime<L, P, C, S>,
) -> Result<(), PinoraError>
where
    L: SingleInstance + 'static,
    P: CapabilityProbe + 'static,
    C: CaptureProvider + 'static,
    S: ImageSink + 'static,
{
    let event_loop = EventLoop::new().map_err(|e| {
        PinoraError::new(ErrorCode::Internal, format!("desktop event loop: {e}"))
    })?;

    let mut app = DesktopApp {
        runtime: Some(runtime),
        context: None,
        mode: Mode::StartCapture,
        overlay: None,
        pins: HashMap::new(),
        drag_pin: None,
        modifiers: ModifiersState::empty(),
        error: None,
        quit: false,
        pending_messages: Vec::new(),
    };

    event_loop
        .run_app(&mut app)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("desktop loop: {e}")))?;

    if let Some(err) = app.error {
        return Err(err);
    }

    if let Some(mut rt) = app.runtime.take() {
        let _ = rt.dispatch(Command::shutdown());
        println!(
            "pinora: shutdown complete (pins={})",
            rt.state().pin_count()
        );
    }
    Ok(())
}

enum Mode {
    /// 下一帧开始全屏捕获并进入选区。
    StartCapture,
    /// 空闲：仅贴图窗口。
    Idle,
}

struct OverlayState {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    base: Vec<u32>,
    dimmed: Vec<u32>,
    frame: Vec<u32>,
    session: SelectionSession,
    dragging: bool,
    last_cursor: PixelPoint,
    needs_redraw: bool,
    last_drawn_rect: Option<PixelRect>,
    last_present: Instant,
    img_w: u32,
    img_h: u32,
    win_w: u32,
    win_h: u32,
    /// 全屏捕获图（用于裁剪）。
    full_image: CaptureImage,
}

struct PinWin {
    pin_id: PinId,
    image: CaptureImage,
    pixels_xrgb: Vec<u32>,
    scale: f64,
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
}

struct DesktopApp<L, P, C, S> {
    runtime: Option<AppRuntime<L, P, C, S>>,
    context: Option<Context<Rc<Window>>>,
    mode: Mode,
    overlay: Option<OverlayState>,
    pins: HashMap<WindowId, PinWin>,
    drag_pin: Option<WindowId>,
    modifiers: ModifiersState,
    error: Option<PinoraError>,
    quit: bool,
    pending_messages: Vec<String>,
}

impl<L, P, C, S> ApplicationHandler for DesktopApp<L, P, C, S>
where
    L: SingleInstance,
    P: CapabilityProbe,
    C: CaptureProvider,
    S: ImageSink,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.ensure_context(event_loop);
        if matches!(self.mode, Mode::StartCapture) && self.overlay.is_none() {
            if let Err(e) = self.begin_region_capture(event_loop) {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // 单实例激活
        if let Some(rt) = self.runtime.as_mut() {
            if let Ok(n) = rt.poll_forwarded() {
                if n > 0 {
                    self.pending_messages.push(format!(
                        "pinora: activated (count={})",
                        rt.state().activation_count
                    ));
                }
            }
        }
        for msg in self.pending_messages.drain(..) {
            println!("{msg}");
        }

        if self.quit {
            event_loop.exit();
            return;
        }

        // Overlay 帧合并
        if let Some(ov) = self.overlay.as_mut() {
            if ov.needs_redraw {
                let elapsed = ov.last_present.elapsed();
                if elapsed >= MIN_FRAME_INTERVAL {
                    ov.needs_redraw = false;
                    ov.window.request_redraw();
                    event_loop.set_control_flow(ControlFlow::Wait);
                } else {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(
                        Instant::now() + (MIN_FRAME_INTERVAL - elapsed),
                    ));
                }
                return;
            }
        }

        if matches!(self.mode, Mode::StartCapture) && self.overlay.is_none() {
            if let Err(e) = self.begin_region_capture(event_loop) {
                self.error = Some(e);
                event_loop.exit();
            }
        }

        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(ov) = self.overlay.as_ref() {
            if ov.window.id() == window_id {
                self.handle_overlay_event(event_loop, event);
                return;
            }
        }
        self.handle_pin_event(event_loop, window_id, event);
    }
}

impl<L, P, C, S> DesktopApp<L, P, C, S>
where
    L: SingleInstance,
    P: CapabilityProbe,
    C: CaptureProvider,
    S: ImageSink,
{
    fn ensure_context(&mut self, event_loop: &ActiveEventLoop) {
        if self.context.is_some() {
            return;
        }
        // 先建一个隐藏占位窗以拿到 display handle（Wayland 需要）
        let attrs = Window::default_attributes()
            .with_visible(false)
            .with_title("pinora-display-handle");
        if let Ok(w) = event_loop.create_window(attrs) {
            let w = Rc::new(w);
            if let Ok(ctx) = Context::new(w) {
                self.context = Some(ctx);
            }
        }
    }

    fn begin_region_capture(&mut self, event_loop: &ActiveEventLoop) -> Result<(), PinoraError> {
        let rt = self.runtime.as_ref().unwrap();
        let capture = rt.capture_provider();
        let displays = capture.displays()?;
        let display = displays.first().ok_or_else(|| {
            PinoraError::new(ErrorCode::NotFound, "no display for region capture")
        })?;

        println!(
            "pinora: capturing {} for overlay ({}x{}) …",
            display.name, display.bounds.size.width, display.bounds.size.height
        );

        let full_image = capture.capture(CaptureRequest::FullDisplay {
            display: display.id.clone(),
        })?;

        let img_w = full_image.pixels.size.width;
        let img_h = full_image.pixels.size.height;
        let base = rgba_to_xrgb(&full_image.pixels.bytes);
        let dimmed: Vec<u32> = base.iter().copied().map(darken).collect();
        let frame = dimmed.clone();

        self.ensure_context(event_loop);
        let context = self.context.as_ref().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "softbuffer context unavailable")
        })?;

        let attrs = Window::default_attributes()
            .with_title("Pinora — 拖拽选区，Enter 确认，Esc 取消")
            .with_inner_size(PhysicalSize::new(img_w, img_h))
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .with_cursor(CursorIcon::Crosshair)
            .with_decorations(false)
            .with_visible(true);

        let window = event_loop
            .create_window(attrs)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("overlay window: {e}")))?;
        let window = Rc::new(window);
        let _ = window.focus_window();

        let mut surface = Surface::new(context, window.clone()).map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("overlay surface: {e}"))
        })?;
        let size = window.inner_size();
        let win_w = size.width.max(1);
        let win_h = size.height.max(1);
        if let (Some(w), Some(h)) = (NonZeroU32::new(win_w), NonZeroU32::new(win_h)) {
            let _ = surface.resize(w, h);
        }

        println!("pinora: drag to select — Enter confirm, Esc cancel");
        self.overlay = Some(OverlayState {
            window: window.clone(),
            surface,
            base,
            dimmed,
            frame,
            session: SelectionSession::new()
                .with_bounds(PixelRect::new(0, 0, img_w, img_h))
                .with_min_edge(2),
            dragging: false,
            last_cursor: PixelPoint::new(0, 0),
            needs_redraw: true,
            last_drawn_rect: None,
            last_present: Instant::now()
                .checked_sub(MIN_FRAME_INTERVAL * 2)
                .unwrap_or_else(Instant::now),
            img_w,
            img_h,
            win_w,
            win_h,
            full_image,
        });
        self.mode = Mode::Idle;
        window.request_redraw();
        Ok(())
    }

    fn handle_overlay_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        let Some(ov) = self.overlay.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                self.cancel_overlay();
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                let step = if self.modifiers.shift_key() { 10 } else { 1 };
                match &event.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        self.cancel_overlay();
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        match ov.session.try_confirm() {
                            Ok(rect) => {
                                if let Err(e) = self.confirm_overlay(event_loop, rect) {
                                    self.error = Some(e);
                                    event_loop.exit();
                                }
                            }
                            Err(_) => ov.needs_redraw = true,
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        ov.session.nudge(-step, 0);
                        ov.needs_redraw = true;
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        ov.session.nudge(step, 0);
                        ov.needs_redraw = true;
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        ov.session.nudge(0, -step);
                        ov.needs_redraw = true;
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        ov.session.nudge(0, step);
                        ov.needs_redraw = true;
                    }
                    _ => {
                        if self.is_quit_key(&event) {
                            self.quit = true;
                            event_loop.exit();
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                ElementState::Pressed => {
                    ov.dragging = true;
                    ov.session.begin_drag(ov.last_cursor);
                    ov.needs_redraw = true;
                }
                ElementState::Released => {
                    ov.dragging = false;
                    ov.needs_redraw = true;
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                ov.last_cursor = window_to_image(
                    position.x,
                    position.y,
                    ov.win_w,
                    ov.win_h,
                    ov.img_w,
                    ov.img_h,
                );
                if ov.dragging {
                    ov.session.update_cursor(ov.last_cursor);
                    ov.needs_redraw = true;
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = paint_overlay(ov) {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                ov.win_w = size.width.max(1);
                ov.win_h = size.height.max(1);
                if let (Some(w), Some(h)) = (NonZeroU32::new(ov.win_w), NonZeroU32::new(ov.win_h))
                {
                    let _ = ov.surface.resize(w, h);
                }
                ov.last_drawn_rect = None;
                ov.frame.copy_from_slice(&ov.dimmed);
                ov.needs_redraw = true;
            }
            _ => {}
        }
    }

    fn cancel_overlay(&mut self) {
        if let Some(ov) = self.overlay.take() {
            ov.window.set_visible(false);
        }
        println!("pinora: selection cancelled");
        if self.pins.is_empty() {
            // 无贴图时取消则再开一轮选区；用户可用 Ctrl+Q 退出
            self.mode = Mode::StartCapture;
        } else {
            self.mode = Mode::Idle;
            // 聚焦任一贴图
            if let Some(pin) = self.pins.values().next() {
                let _ = pin.window.focus_window();
            }
        }
    }

    fn confirm_overlay(
        &mut self,
        event_loop: &ActiveEventLoop,
        local: PixelRect,
    ) -> Result<(), PinoraError> {
        let ov = self.overlay.take().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "overlay missing on confirm")
        })?;
        ov.window.set_visible(false);

        let cropped = ov.full_image.crop_local(local)?;
        let size = cropped.size();
        let position = PixelPoint::new(
            cropped.source_rect.origin.x.saturating_add(24),
            cropped.source_rect.origin.y.saturating_add(24),
        );

        let rt = self.runtime.as_mut().unwrap();
        let pin = rt.dispatch(Command::create_pin(cropped.clone(), position))?;
        let pin_id = pin
            .events
            .iter()
            .find_map(|e| match e.event.kind {
                DomainEventKind::PinCreated { pin_id, .. } => Some(pin_id),
                _ => None,
            })
            .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "missing PinCreated"))?;

        println!(
            "pinora: pin {pin_id} ({}x{}) — drag move, scroll zoom, Esc close, F2/Ctrl+N 再截",
            size.width, size.height
        );

        if let Ok(save) = rt.dispatch(Command::invoke_action(ActionId::SaveLastCapture)) {
            for event in &save.events {
                if let DomainEventKind::ImageSaved { image_id, path } = &event.event.kind {
                    println!("pinora: saved {image_id} -> {}", path.display());
                }
            }
        }
        let _ = rt.dispatch(Command::invoke_action(ActionId::CopyLastCapture));

        self.spawn_pin(
            event_loop,
            pin_id,
            cropped,
            position,
            1.0,
        )?;
        self.mode = Mode::Idle;
        Ok(())
    }

    fn spawn_pin(
        &mut self,
        event_loop: &ActiveEventLoop,
        pin_id: PinId,
        image: CaptureImage,
        position: PixelPoint,
        scale: f64,
    ) -> Result<(), PinoraError> {
        self.ensure_context(event_loop);
        let context = self.context.as_ref().ok_or_else(|| {
            PinoraError::new(ErrorCode::Internal, "softbuffer context unavailable")
        })?;

        let (w, h) = scaled_window_size(image.size(), scale);
        let pixels_xrgb = rgba_to_xrgb(&image.pixels.bytes);

        let attrs = Window::default_attributes()
            .with_title(format!("Pinora {pin_id}"))
            .with_inner_size(PhysicalSize::new(w, h))
            .with_position(PhysicalPosition::new(position.x, position.y))
            .with_decorations(false)
            .with_resizable(true)
            .with_visible(true)
            .with_window_level(WindowLevel::AlwaysOnTop);

        let window = event_loop
            .create_window(attrs)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("pin window: {e}")))?;
        let window = Rc::new(window);
        let _ = window.focus_window();

        let mut surface = Surface::new(context, window.clone()).map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("pin surface: {e}"))
        })?;
        if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
            let _ = surface.resize(nw, nh);
        }

        let id = window.id();
        window.request_redraw();
        self.pins.insert(
            id,
            PinWin {
                pin_id,
                image,
                pixels_xrgb,
                scale,
                window,
                surface,
            },
        );
        println!("pinora: pin window visible ({w}x{h})");
        Ok(())
    }

    fn handle_pin_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.close_pin(window_id);
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                if self.is_quit_key(&event) {
                    self.quit = true;
                    event_loop.exit();
                    return;
                }
                if self.is_new_capture_key(&event) {
                    println!("pinora: new capture requested");
                    self.mode = Mode::StartCapture;
                    return;
                }
                if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                    self.close_pin(window_id);
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                // Wayland 下用协议级拖动，set_outer_position 往往无效
                if let Some(pin) = self.pins.get(&window_id) {
                    if let Err(e) = pin.window.drag_window() {
                        eprintln!("pinora: drag_window failed: {e:?}");
                        self.drag_pin = Some(window_id);
                    } else {
                        self.drag_pin = None;
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.drag_pin = None;
            }
            WindowEvent::CursorMoved { position, .. } => {
                // 回退拖动（X11 / drag_window 失败时）
                let Some(id) = self.drag_pin else {
                    return;
                };
                if id != window_id {
                    return;
                }
                let Some(pin) = self.pins.get(&window_id) else {
                    return;
                };
                if let Ok(outer) = pin.window.outer_position() {
                    // 简单跟随：将窗口左上角移到近似位置
                    let _ = pin.window.set_outer_position(PhysicalPosition::new(
                        outer.x + position.x as i32 / 8,
                        outer.y + position.y as i32 / 8,
                    ));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y / 50.0,
                };
                if steps.abs() < f64::EPSILON {
                    return;
                }
                let Some(pin) = self.pins.get_mut(&window_id) else {
                    return;
                };
                let factor = if steps > 0.0 { 1.1_f64 } else { 1.0 / 1.1 };
                pin.scale = (pin.scale * factor).clamp(0.1, 8.0);
                let (w, h) = scaled_window_size(pin.image.size(), pin.scale);
                let _ = pin.window.request_inner_size(PhysicalSize::new(w, h));
                if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                    let _ = pin.surface.resize(nw, nh);
                }
                pin.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.paint_pin(window_id) {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(pin) = self.pins.get_mut(&window_id) {
                    if let (Some(w), Some(h)) = (
                        NonZeroU32::new(size.width.max(1)),
                        NonZeroU32::new(size.height.max(1)),
                    ) {
                        let _ = pin.surface.resize(w, h);
                    }
                    pin.window.request_redraw();
                }
            }
            WindowEvent::Focused(true) => {
                // 确保键盘可用
            }
            _ => {}
        }
    }

    fn close_pin(&mut self, window_id: WindowId) {
        if let Some(pin) = self.pins.remove(&window_id) {
            println!("pinora: pin {} closed", pin.pin_id);
            if let Some(rt) = self.runtime.as_mut() {
                let _ = rt.dispatch(Command::close_pin(pin.pin_id));
            }
        }
        self.drag_pin = None;
        if self.pins.is_empty() && self.overlay.is_none() {
            println!("pinora: all pins closed — starting new capture (Ctrl+Q to quit)");
            self.mode = Mode::StartCapture;
        }
    }

    fn paint_pin(&mut self, window_id: WindowId) -> Result<(), PinoraError> {
        let Some(pin) = self.pins.get_mut(&window_id) else {
            return Ok(());
        };
        let size = pin.window.inner_size();
        let bw = size.width.max(1) as usize;
        let bh = size.height.max(1) as usize;
        let mut buffer = pin.surface.buffer_mut().map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("pin buffer: {e}"))
        })?;
        if buffer.len() < bw * bh {
            return Err(PinoraError::new(
                ErrorCode::Internal,
                "pin buffer size mismatch",
            ));
        }
        let sw = pin.image.pixels.size.width as usize;
        let sh = pin.image.pixels.size.height as usize;
        if bw == sw && bh == sh {
            buffer[..bw * bh].copy_from_slice(&pin.pixels_xrgb);
        } else {
            scale_nearest(&pin.pixels_xrgb, sw, sh, &mut buffer[..bw * bh], bw, bh);
        }
        draw_border(&mut buffer[..bw * bh], bw, bh, 0x00_40_A0_FF);
        buffer
            .present()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("pin present: {e}")))?;
        Ok(())
    }

    fn is_quit_key(&self, event: &winit::event::KeyEvent) -> bool {
        self.modifiers.control_key()
            && matches!(
                event.physical_key,
                PhysicalKey::Code(KeyCode::KeyQ)
            )
    }

    fn is_new_capture_key(&self, event: &winit::event::KeyEvent) -> bool {
        matches!(event.logical_key, Key::Named(NamedKey::F2))
            || (self.modifiers.control_key()
                && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyN)))
    }
}

fn paint_overlay(ov: &mut OverlayState) -> Result<(), PinoraError> {
    let img_w = ov.img_w as usize;
    let img_h = ov.img_h as usize;
    let new_rect = ov.session.preview_rect();

    if ov.last_drawn_rect != new_rect {
        if let Some(old) = ov.last_drawn_rect {
            blit_rect(&mut ov.frame, &ov.dimmed, img_w, img_h, old);
        }
        if let Some(rect) = new_rect {
            blit_rect(&mut ov.frame, &ov.base, img_w, img_h, rect);
            let x0 = rect.origin.x.max(0) as usize;
            let y0 = rect.origin.y.max(0) as usize;
            let x1 = (rect.right() as usize).min(img_w);
            let y1 = (rect.bottom() as usize).min(img_h);
            draw_rect_border(&mut ov.frame, img_w, img_h, x0, y0, x1, y1, 0x00_FF_CC_33);
        }
        ov.last_drawn_rect = new_rect;
    }

    let mut buffer = ov.surface.buffer_mut().map_err(|e| {
        PinoraError::new(ErrorCode::Internal, format!("overlay buffer: {e}"))
    })?;
    let bw = ov.win_w as usize;
    let bh = ov.win_h as usize;
    let needed = bw * bh;
    if buffer.len() < needed {
        return Err(PinoraError::new(
            ErrorCode::Internal,
            "overlay buffer size mismatch",
        ));
    }
    if bw == img_w && bh == img_h {
        buffer[..needed].copy_from_slice(&ov.frame);
    } else {
        scale_nearest(&ov.frame, img_w, img_h, &mut buffer[..needed], bw, bh);
    }
    buffer
        .present()
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("overlay present: {e}")))?;
    ov.last_present = Instant::now();
    Ok(())
}

fn window_to_image(x: f64, y: f64, win_w: u32, win_h: u32, img_w: u32, img_h: u32) -> PixelPoint {
    let ix = if win_w == 0 {
        0
    } else {
        ((x * f64::from(img_w)) / f64::from(win_w)).round() as i32
    };
    let iy = if win_h == 0 {
        0
    } else {
        ((y * f64::from(img_h)) / f64::from(win_h)).round() as i32
    };
    PixelPoint::new(
        ix.clamp(0, img_w.saturating_sub(1) as i32),
        iy.clamp(0, img_h.saturating_sub(1) as i32),
    )
}

fn rgba_to_xrgb(bytes: &[u8]) -> Vec<u32> {
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for c in bytes.chunks_exact(4) {
        out.push((u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]));
    }
    out
}

fn darken(c: u32) -> u32 {
    let r = ((c >> 16) & 0xff) * 2 / 5;
    let g = ((c >> 8) & 0xff) * 2 / 5;
    let b = (c & 0xff) * 2 / 5;
    (r << 16) | (g << 8) | b
}

fn blit_rect(dst: &mut [u32], src: &[u32], stride: usize, height: usize, rect: PixelRect) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right() as usize).min(stride);
    let y1 = (rect.bottom() as usize).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let row_w = x1 - x0;
    for y in y0..y1 {
        let start = y * stride + x0;
        dst[start..start + row_w].copy_from_slice(&src[start..start + row_w]);
    }
}

fn scale_nearest(src: &[u32], sw: usize, sh: usize, dst: &mut [u32], dw: usize, dh: usize) {
    for y in 0..dh {
        let sy = y * sh / dh;
        let src_row = sy * sw;
        let dst_row = y * dw;
        for x in 0..dw {
            dst[dst_row + x] = src[src_row + x * sw / dw];
        }
    }
}

fn draw_rect_border(
    buf: &mut [u32],
    stride: usize,
    height: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    color: u32,
) {
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for t in 0..2usize {
        let yt = y0 + t;
        let yb = y1.saturating_sub(1 + t);
        if yt < height {
            for x in x0..x1 {
                buf[yt * stride + x] = color;
            }
        }
        if yb < height && yb >= y0 {
            for x in x0..x1 {
                buf[yb * stride + x] = color;
            }
        }
        let xl = x0 + t;
        let xr = x1.saturating_sub(1 + t);
        for y in y0..y1 {
            if xl < stride {
                buf[y * stride + xl] = color;
            }
            if xr < stride {
                buf[y * stride + xr] = color;
            }
        }
    }
}

fn draw_border(buf: &mut [u32], w: usize, h: usize, color: u32) {
    if w == 0 || h == 0 {
        return;
    }
    for x in 0..w {
        buf[x] = color;
        buf[(h - 1) * w + x] = color;
    }
    for y in 0..h {
        buf[y * w] = color;
        buf[y * w + w - 1] = color;
    }
}
