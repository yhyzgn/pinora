//! 贴图窗口：无边框、置顶、拖动、滚轮缩放、Esc 关闭。

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;

use pinora_core::{CaptureImage, ErrorCode, PinId, PinoraError, PixelPoint, PixelSize};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId, WindowLevel};

/// 贴图桌面会话结束原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinSessionEnd {
    /// 所有贴图已关闭。
    AllClosed,
    /// 用户请求再截一张（Ctrl+N / F2）。
    NewCapture,
    /// 用户请求退出应用（Ctrl+Q）。
    Quit,
}

/// 待显示的贴图描述。
#[derive(Debug, Clone)]
pub struct PinView {
    pub pin_id: PinId,
    pub image: CaptureImage,
    pub position: PixelPoint,
    pub scale: f64,
}

/// 根据图像尺寸与缩放计算窗口物理像素大小。
pub fn scaled_window_size(image: PixelSize, scale: f64) -> (u32, u32) {
    let s = if scale.is_finite() && scale > 0.0 {
        scale.clamp(0.05, 8.0)
    } else {
        1.0
    };
    let w = ((f64::from(image.width) * s).round() as u32).max(1);
    let h = ((f64::from(image.height) * s).round() as u32).max(1);
    (w, h)
}

/// 运行贴图窗口事件循环，直到全部关闭或用户发出再截/退出。
///
/// 返回结束原因，以及本会话中被关闭的 `PinId` 列表（用于同步 runtime）。
pub fn run_pin_session(pins: Vec<PinView>) -> Result<(PinSessionEnd, Vec<PinId>), PinoraError> {
    if pins.is_empty() {
        return Ok((PinSessionEnd::AllClosed, Vec::new()));
    }

    let event_loop = EventLoop::new()
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("pin event loop: {e}")))?;

    let mut app = PinDesktopApp {
        pending: pins,
        windows: HashMap::new(),
        context: None,
        drag: None,
        modifiers: ModifiersState::empty(),
        end: None,
        closed: Vec::new(),
        error: None,
    };

    event_loop
        .run_app(&mut app)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("pin loop: {e}")))?;

    if let Some(err) = app.error {
        return Err(err);
    }

    Ok((app.end.unwrap_or(PinSessionEnd::AllClosed), app.closed))
}

struct PinWindowState {
    pin_id: PinId,
    image: CaptureImage,
    pixels_xrgb: Vec<u32>,
    scale: f64,
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
}

struct DragState {
    window_id: WindowId,
    /// 按下时指针在窗口客户区内的位置。
    grab_x: f64,
    grab_y: f64,
}

struct PinDesktopApp {
    pending: Vec<PinView>,
    windows: HashMap<WindowId, PinWindowState>,
    context: Option<Context<Rc<Window>>>,
    drag: Option<DragState>,
    modifiers: ModifiersState,
    end: Option<PinSessionEnd>,
    closed: Vec<PinId>,
    error: Option<PinoraError>,
}

impl ApplicationHandler for PinDesktopApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let pending = std::mem::take(&mut self.pending);
        for view in pending {
            if let Err(e) = self.spawn_pin(event_loop, view) {
                self.error = Some(e);
                event_loop.exit();
                return;
            }
        }
        if self.windows.is_empty() {
            self.end = Some(PinSessionEnd::AllClosed);
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        event_loop.set_control_flow(ControlFlow::Wait);

        match event {
            WindowEvent::CloseRequested => {
                self.close_window(window_id, event_loop);
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                match &event.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        self.close_window(window_id, event_loop);
                    }
                    Key::Named(NamedKey::F2) => {
                        self.end = Some(PinSessionEnd::NewCapture);
                        event_loop.exit();
                    }
                    Key::Character(c) if self.modifiers.control_key() && (c == "n" || c == "N") => {
                        self.end = Some(PinSessionEnd::NewCapture);
                        event_loop.exit();
                    }
                    Key::Character(c) if self.modifiers.control_key() && (c == "q" || c == "Q") => {
                        self.end = Some(PinSessionEnd::Quit);
                        event_loop.exit();
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                ElementState::Pressed => {
                    // grab 在 CursorMoved 里用最新指针位置初始化
                    self.drag = Some(DragState {
                        window_id,
                        grab_x: f64::NAN,
                        grab_y: f64::NAN,
                    });
                }
                ElementState::Released => {
                    self.drag = None;
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                let Some(drag) = self.drag.as_mut() else {
                    return;
                };
                if drag.window_id != window_id {
                    return;
                }
                if drag.grab_x.is_nan() {
                    drag.grab_x = position.x;
                    drag.grab_y = position.y;
                    return;
                }
                let Some(state) = self.windows.get(&window_id) else {
                    return;
                };
                let Ok(outer) = state.window.outer_position() else {
                    return;
                };
                // outer' = outer + (cursor_client - grab)
                let new_x = f64::from(outer.x) + (position.x - drag.grab_x);
                let new_y = f64::from(outer.y) + (position.y - drag.grab_y);
                state.window.set_outer_position(PhysicalPosition::new(
                    new_x.round() as i32,
                    new_y.round() as i32,
                ));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y / 50.0,
                };
                if steps.abs() < f64::EPSILON {
                    return;
                }
                let Some(state) = self.windows.get_mut(&window_id) else {
                    return;
                };
                let factor = if steps > 0.0 { 1.1_f64 } else { 1.0 / 1.1 };
                state.scale = (state.scale * factor).clamp(0.1, 8.0);
                let (w, h) = scaled_window_size(state.image.size(), state.scale);
                let _ = state.window.request_inner_size(PhysicalSize::new(w, h));
                if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                    let _ = state.surface.resize(nw, nh);
                }
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.paint(window_id) {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(state) = self.windows.get_mut(&window_id) {
                    if let (Some(w), Some(h)) = (
                        NonZeroU32::new(size.width.max(1)),
                        NonZeroU32::new(size.height.max(1)),
                    ) {
                        let _ = state.surface.resize(w, h);
                    }
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

impl PinDesktopApp {
    fn spawn_pin(
        &mut self,
        event_loop: &ActiveEventLoop,
        view: PinView,
    ) -> Result<(), PinoraError> {
        let (w, h) = scaled_window_size(view.image.size(), view.scale);
        let pixels_xrgb = rgba_to_xrgb(&view.image.pixels.bytes);

        let attrs = Window::default_attributes()
            .with_title(format!("Pinora pin {}", view.pin_id))
            .with_inner_size(PhysicalSize::new(w, h))
            .with_position(PhysicalPosition::new(view.position.x, view.position.y))
            .with_decorations(false)
            .with_resizable(true)
            .with_window_level(WindowLevel::AlwaysOnTop);

        let window = event_loop.create_window(attrs).map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("create pin window: {e}"))
        })?;
        let window = Rc::new(window);

        if self.context.is_none() {
            let ctx = Context::new(window.clone()).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("softbuffer context: {e}"))
            })?;
            self.context = Some(ctx);
        }
        let context = self.context.as_ref().unwrap();
        let mut surface = Surface::new(context, window.clone()).map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("softbuffer surface: {e}"))
        })?;
        if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
            let _ = surface.resize(nw, nh);
        }

        let id = window.id();
        window.request_redraw();
        self.windows.insert(
            id,
            PinWindowState {
                pin_id: view.pin_id,
                image: view.image,
                pixels_xrgb,
                scale: view.scale,
                window,
                surface,
            },
        );
        println!("pinora: pin window opened ({w}x{h}) — drag to move, scroll to zoom, Esc close");
        println!("pinora:   Ctrl+N / F2 再截图，Ctrl+Q 退出");
        Ok(())
    }

    fn close_window(&mut self, window_id: WindowId, event_loop: &ActiveEventLoop) {
        if let Some(state) = self.windows.remove(&window_id) {
            println!("pinora: pin {} closed", state.pin_id);
            self.closed.push(state.pin_id);
        }
        self.drag = None;
        if self.windows.is_empty() && self.end.is_none() {
            self.end = Some(PinSessionEnd::AllClosed);
            event_loop.exit();
        }
    }

    fn paint(&mut self, window_id: WindowId) -> Result<(), PinoraError> {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return Ok(());
        };
        let size = state.window.inner_size();
        let bw = size.width.max(1) as usize;
        let bh = size.height.max(1) as usize;

        let mut buffer = state
            .surface
            .buffer_mut()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("pin buffer: {e}")))?;
        if buffer.len() < bw * bh {
            return Err(PinoraError::new(
                ErrorCode::Internal,
                "pin softbuffer size mismatch",
            ));
        }

        let sw = state.image.pixels.size.width as usize;
        let sh = state.image.pixels.size.height as usize;
        if bw == sw && bh == sh {
            buffer[..bw * bh].copy_from_slice(&state.pixels_xrgb);
        } else {
            scale_nearest(&state.pixels_xrgb, sw, sh, &mut buffer[..bw * bh], bw, bh);
        }

        draw_border(&mut buffer[..bw * bh], bw, bh, 0x00_40_A0_FF);

        buffer
            .present()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("pin present: {e}")))?;
        Ok(())
    }
}

fn rgba_to_xrgb(bytes: &[u8]) -> Vec<u32> {
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for c in bytes.chunks_exact(4) {
        out.push((u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]));
    }
    out
}

fn scale_nearest(src: &[u32], sw: usize, sh: usize, dst: &mut [u32], dw: usize, dh: usize) {
    for y in 0..dh {
        let sy = y * sh / dh;
        let src_row = sy * sw;
        let dst_row = y * dw;
        for x in 0..dw {
            let sx = x * sw / dw;
            dst[dst_row + x] = src[src_row + sx];
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
        buf[y * w + (w - 1)] = color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::PixelSize;

    #[test]
    fn scaled_window_size_basic() {
        let (w, h) = scaled_window_size(PixelSize::new(100, 50), 2.0);
        assert_eq!((w, h), (200, 100));
    }

    #[test]
    fn scaled_window_size_min_clamp() {
        let (w, h) = scaled_window_size(PixelSize::new(100, 50), 0.01);
        assert_eq!((w, h), (5, 3));
    }
}
