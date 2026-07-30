//! 区域选区 Overlay：全屏显示背景截图，拖拽选区，Enter 确认 / Esc 取消。

use std::num::NonZeroU32;
use std::rc::Rc;

use pinora_core::{
    CaptureImage, ErrorCode, PinoraError, PixelPoint, PixelRect, SelectionSession,
};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Fullscreen, Window, WindowId};

/// 打开交互选区；返回图像本地坐标选区，或 `None`（取消）。
pub fn run_region_selection(background: &CaptureImage) -> Result<Option<PixelRect>, PinoraError> {
    let width = background.pixels.size.width;
    let height = background.pixels.size.height;
    if width == 0 || height == 0 {
        return Err(PinoraError::new(
            ErrorCode::CommandRejected,
            "background image is empty",
        ));
    }

    let base = rgba_to_xrgb(&background.pixels.bytes);
    let event_loop = EventLoop::new().map_err(|e| {
        PinoraError::new(ErrorCode::Internal, format!("event loop: {e}"))
    })?;

    let mut app = OverlayApp {
        width,
        height,
        base,
        session: SelectionSession::new()
            .with_bounds(PixelRect::new(0, 0, width, height))
            .with_min_edge(2),
        dragging: false,
        last_cursor: PixelPoint::new(0, 0),
        modifiers: ModifiersState::empty(),
        window: None,
        context: None,
        surface: None,
        result: OverlayResult::Running,
        error: None,
    };

    event_loop
        .run_app(&mut app)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("overlay loop: {e}")))?;

    if let Some(err) = app.error {
        return Err(err);
    }

    match app.result {
        OverlayResult::Confirmed(rect) => Ok(Some(rect)),
        OverlayResult::Cancelled | OverlayResult::Running => Ok(None),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OverlayResult {
    Running,
    Confirmed(PixelRect),
    Cancelled,
}

struct OverlayApp {
    width: u32,
    height: u32,
    base: Vec<u32>,
    session: SelectionSession,
    dragging: bool,
    last_cursor: PixelPoint,
    modifiers: ModifiersState,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    result: OverlayResult,
    error: Option<PinoraError>,
}

impl ApplicationHandler for OverlayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("Pinora — 拖拽选区，Enter 确认，Esc 取消")
            .with_inner_size(PhysicalSize::new(self.width, self.height))
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .with_cursor(CursorIcon::Crosshair)
            .with_decorations(false);

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                self.error = Some(PinoraError::new(
                    ErrorCode::Internal,
                    format!("create overlay window: {e}"),
                ));
                event_loop.exit();
                return;
            }
        };

        let context = match Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                self.error = Some(PinoraError::new(
                    ErrorCode::Internal,
                    format!("softbuffer context: {e}"),
                ));
                event_loop.exit();
                return;
            }
        };

        let mut surface = match Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                self.error = Some(PinoraError::new(
                    ErrorCode::Internal,
                    format!("softbuffer surface: {e}"),
                ));
                event_loop.exit();
                return;
            }
        };

        if let (Some(w), Some(h)) = (NonZeroU32::new(self.width), NonZeroU32::new(self.height)) {
            let _ = surface.resize(w, h);
        }

        self.context = Some(context);
        self.surface = Some(surface);
        self.window = Some(window.clone());
        window.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        event_loop.set_control_flow(ControlFlow::Wait);

        match event {
            WindowEvent::CloseRequested => {
                self.result = OverlayResult::Cancelled;
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                let step = if self.modifiers.shift_key() { 10 } else { 1 };
                match &event.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        self.result = OverlayResult::Cancelled;
                        event_loop.exit();
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        match self.session.try_confirm() {
                            Ok(rect) => {
                                self.result = OverlayResult::Confirmed(rect);
                                event_loop.exit();
                            }
                            Err(_) => window.request_redraw(),
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.session.nudge(-step, 0);
                        window.request_redraw();
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        self.session.nudge(step, 0);
                        window.request_redraw();
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.session.nudge(0, -step);
                        window.request_redraw();
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.session.nudge(0, step);
                        window.request_redraw();
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
                    self.dragging = true;
                    self.session.begin_drag(self.last_cursor);
                    window.request_redraw();
                }
                ElementState::Released => {
                    self.dragging = false;
                    window.request_redraw();
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor = PixelPoint::new(position.x as i32, position.y as i32);
                if self.dragging {
                    self.session.update_cursor(self.last_cursor);
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.paint() {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(surface) = self.surface.as_mut() {
                    if let (Some(w), Some(h)) = (
                        NonZeroU32::new(size.width.max(1)),
                        NonZeroU32::new(size.height.max(1)),
                    ) {
                        let _ = surface.resize(w, h);
                    }
                }
                window.request_redraw();
            }
            _ => {}
        }
    }
}

impl OverlayApp {
    fn paint(&mut self) -> Result<(), PinoraError> {
        let Some(surface) = self.surface.as_mut() else {
            return Ok(());
        };
        let mut buffer = surface.buffer_mut().map_err(|e| {
            PinoraError::new(ErrorCode::Internal, format!("buffer_mut: {e}"))
        })?;

        let w = self.width as usize;
        let h = self.height as usize;
        let needed = w * h;
        if buffer.len() < needed {
            return Err(PinoraError::new(
                ErrorCode::Internal,
                "softbuffer size mismatch",
            ));
        }

        // 暗色遮罩底图
        for i in 0..needed {
            buffer[i] = darken(self.base.get(i).copied().unwrap_or(0));
        }

        // 选区恢复原图亮度 + 边框
        if let Some(rect) = self.session.preview_rect() {
            let x0 = rect.origin.x.max(0) as usize;
            let y0 = rect.origin.y.max(0) as usize;
            let x1 = (rect.right() as usize).min(w);
            let y1 = (rect.bottom() as usize).min(h);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * w + x;
                    if let Some(px) = self.base.get(i) {
                        buffer[i] = *px;
                    }
                }
            }
            let border = 0x00_FF_CC_33u32;
            draw_rect_border(&mut buffer, w, h, x0, y0, x1, y1, border);
        }

        buffer
            .present()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("present: {e}")))?;
        Ok(())
    }
}

fn rgba_to_xrgb(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| {
            let r = u32::from(c[0]);
            let g = u32::from(c[1]);
            let b = u32::from(c[2]);
            (r << 16) | (g << 8) | b
        })
        .collect()
}

fn darken(c: u32) -> u32 {
    let r = ((c >> 16) & 0xff) * 2 / 5;
    let g = ((c >> 8) & 0xff) * 2 / 5;
    let b = (c & 0xff) * 2 / 5;
    (r << 16) | (g << 8) | b
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
    for x in x0..x1 {
        put(buf, stride, height, x, y0, color);
        put(buf, stride, height, x, y1.saturating_sub(1), color);
    }
    for y in y0..y1 {
        put(buf, stride, height, x0, y, color);
        put(buf, stride, height, x1.saturating_sub(1), y, color);
    }
}

fn put(buf: &mut [u32], stride: usize, height: usize, x: usize, y: usize, color: u32) {
    if x < stride && y < height {
        buf[y * stride + x] = color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_conversion_round_components() {
        let xrgb = rgba_to_xrgb(&[0xff, 0x80, 0x00, 0xff]);
        assert_eq!(xrgb[0], 0x00ff8000);
        assert_eq!(darken(0x00ff8000) & 0xff, (0x00 * 2 / 5));
    }
}
