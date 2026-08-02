//! 区域选区 Overlay：全屏显示背景截图，拖拽选区，Enter 确认 / Esc 取消。
//!
//! 性能要点：
//! - 启动时预计算暗色底图（避免每帧全屏 darken）
//! - 脏矩形：只恢复上一选区 / 绘制当前选区
//! - 鼠标移动只标脏，在 `about_to_wait` 合并为每帧最多一次 redraw/present

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use pinora_core::{CaptureImage, ErrorCode, PinoraError, PixelPoint, PixelRect, SelectionSession};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Fullscreen, Window, WindowId};

use crate::window_policy::{self, AuxiliaryWindowKind};

/// 帧间隔下限（约 60fps），避免 present 堆积造成卡顿。
const MIN_FRAME_INTERVAL: Duration = Duration::from_micros(16_666);

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
    let dimmed: Vec<u32> = base.iter().copied().map(darken).collect();
    let frame = dimmed.clone();

    let event_loop = window_policy::auxiliary_event_loop()
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("event loop: {e}")))?;

    let mut app = OverlayApp {
        width,
        height,
        base,
        dimmed,
        frame,
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
        needs_redraw: true,
        last_drawn_rect: None,
        last_present: Instant::now()
            .checked_sub(MIN_FRAME_INTERVAL * 2)
            .unwrap_or_else(Instant::now),
        win_w: width,
        win_h: height,
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
    dimmed: Vec<u32>,
    frame: Vec<u32>,
    session: SelectionSession,
    dragging: bool,
    last_cursor: PixelPoint,
    modifiers: ModifiersState,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    result: OverlayResult,
    error: Option<PinoraError>,
    needs_redraw: bool,
    last_drawn_rect: Option<PixelRect>,
    last_present: Instant,
    win_w: u32,
    win_h: u32,
}

impl ApplicationHandler for OverlayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let title = "Pinora — 拖拽选区，Enter 确认，Esc 取消";
        let attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(PhysicalSize::new(self.width, self.height))
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .with_cursor(CursorIcon::Crosshair)
            .with_decorations(false);

        let window = match window_policy::create_auxiliary_window(
            event_loop,
            AuxiliaryWindowKind::Overlay,
            attrs,
        ) {
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
        window_policy::apply_post_map_policy(AuxiliaryWindowKind::Overlay, title);

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

        let size = window.inner_size();
        self.win_w = size.width.max(1);
        self.win_h = size.height.max(1);
        if let (Some(w), Some(h)) = (NonZeroU32::new(self.win_w), NonZeroU32::new(self.win_h)) {
            let _ = surface.resize(w, h);
        }

        self.context = Some(context);
        self.surface = Some(surface);
        self.window = Some(window);
        self.needs_redraw = true;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.needs_redraw {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }

        let elapsed = self.last_present.elapsed();
        if elapsed >= MIN_FRAME_INTERVAL {
            self.needs_redraw = false;
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::Wait);
        } else {
            // 等到下一帧时刻再绘，合并期间的鼠标事件
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + (MIN_FRAME_INTERVAL - elapsed),
            ));
        }
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
                            Err(_) => self.needs_redraw = true,
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.session.nudge(-step, 0);
                        self.needs_redraw = true;
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        self.session.nudge(step, 0);
                        self.needs_redraw = true;
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.session.nudge(0, -step);
                        self.needs_redraw = true;
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.session.nudge(0, step);
                        self.needs_redraw = true;
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
                    self.needs_redraw = true;
                }
                ElementState::Released => {
                    self.dragging = false;
                    self.needs_redraw = true;
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor = self.window_to_image(position.x, position.y);
                if self.dragging {
                    self.session.update_cursor(self.last_cursor);
                    self.needs_redraw = true;
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.paint() {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                self.win_w = size.width.max(1);
                self.win_h = size.height.max(1);
                if let Some(surface) = self.surface.as_mut()
                    && let (Some(w), Some(h)) =
                        (NonZeroU32::new(self.win_w), NonZeroU32::new(self.win_h))
                {
                    let _ = surface.resize(w, h);
                }
                self.last_drawn_rect = None;
                self.frame.copy_from_slice(&self.dimmed);
                self.needs_redraw = true;
            }
            _ => {}
        }
    }
}

impl OverlayApp {
    fn window_to_image(&self, x: f64, y: f64) -> PixelPoint {
        let ix = if self.win_w == 0 {
            0
        } else {
            ((x * f64::from(self.width)) / f64::from(self.win_w)).round() as i32
        };
        let iy = if self.win_h == 0 {
            0
        } else {
            ((y * f64::from(self.height)) / f64::from(self.win_h)).round() as i32
        };
        PixelPoint::new(
            ix.clamp(0, self.width.saturating_sub(1) as i32),
            iy.clamp(0, self.height.saturating_sub(1) as i32),
        )
    }

    fn paint(&mut self) -> Result<(), PinoraError> {
        let Some(surface) = self.surface.as_mut() else {
            return Ok(());
        };

        let img_w = self.width as usize;
        let img_h = self.height as usize;
        let new_rect = self.session.preview_rect();

        // 脏矩形更新 CPU 帧缓冲（通常只是两个选区量级的像素）
        if self.last_drawn_rect != new_rect {
            if let Some(old) = self.last_drawn_rect {
                blit_rect(&mut self.frame, &self.dimmed, img_w, img_h, old);
            }
            if let Some(rect) = new_rect {
                blit_rect(&mut self.frame, &self.base, img_w, img_h, rect);
                draw_rect_border(&mut self.frame, img_w, img_h, rect, 0x00_FF_CC_33);
            }
            self.last_drawn_rect = new_rect;
        }

        let mut buffer = surface
            .buffer_mut()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("buffer_mut: {e}")))?;

        let bw = self.win_w as usize;
        let bh = self.win_h as usize;
        let needed = bw * bh;
        if buffer.len() < needed {
            return Err(PinoraError::new(
                ErrorCode::Internal,
                "softbuffer size mismatch",
            ));
        }

        if bw == img_w && bh == img_h {
            buffer[..needed].copy_from_slice(&self.frame);
        } else {
            // 全屏尺寸与截图像素不一致时最近邻缩放（少见；比每帧 darken 便宜）
            scale_nearest(&self.frame, img_w, img_h, &mut buffer[..needed], bw, bh);
        }

        buffer
            .present()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("present: {e}")))?;
        self.last_present = Instant::now();
        Ok(())
    }
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
        let end = start + row_w;
        dst[start..end].copy_from_slice(&src[start..end]);
    }
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

fn rgba_to_xrgb(bytes: &[u8]) -> Vec<u32> {
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for c in bytes.chunks_exact(4) {
        let r = u32::from(c[0]);
        let g = u32::from(c[1]);
        let b = u32::from(c[2]);
        out.push((r << 16) | (g << 8) | b);
    }
    out
}

fn darken(c: u32) -> u32 {
    let r = ((c >> 16) & 0xff) * 2 / 5;
    let g = ((c >> 8) & 0xff) * 2 / 5;
    let b = (c & 0xff) * 2 / 5;
    (r << 16) | (g << 8) | b
}

fn draw_rect_border(buf: &mut [u32], stride: usize, height: usize, rect: PixelRect, color: u32) {
    let x0 = rect.origin.x.max(0) as usize;
    let y0 = rect.origin.y.max(0) as usize;
    let x1 = (rect.right() as usize).min(stride);
    let y1 = (rect.bottom() as usize).min(height);
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
        if xl < stride {
            for y in y0..y1 {
                buf[y * stride + xl] = color;
            }
        }
        if xr < stride && xr >= x0 {
            for y in y0..y1 {
                buf[y * stride + xr] = color;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_conversion_round_components() {
        let xrgb = rgba_to_xrgb(&[0xff, 0x80, 0x00, 0xff]);
        assert_eq!(xrgb[0], 0x00ff8000);
        assert_eq!(darken(0x00ff8000) & 0xff, 0u32);
    }

    #[test]
    fn blit_rect_copies_rows() {
        let mut dst = vec![0u32; 4 * 4];
        let src: Vec<u32> = (0..16).collect();
        blit_rect(&mut dst, &src, 4, 4, PixelRect::new(1, 1, 2, 2));
        assert_eq!(dst[4 + 1], 5);
        assert_eq!(dst[4 + 2], 6);
        assert_eq!(dst[2 * 4 + 1], 9);
        assert_eq!(dst[0], 0);
    }
}
