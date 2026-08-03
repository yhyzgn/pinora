//! Overlay 的 winit/softbuffer 资源适配器。
//!
//! 适配器只拥有当前 Overlay 的窗口与表面，并强制复用统一窗口策略。Overlay 会话、绘制内容、
//! 输入、任务、关闭 owner 和 EventLoop 均由调用方编排。

use std::num::NonZeroU32;
use std::rc::Rc;

use pinora_core::{ErrorCode, PinoraError, PixelSize};
use softbuffer::{Context, Surface};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

use pinora_desktop::window_policy::{self, AuxiliaryWindowKind};

/// 单个 Overlay 的平台窗口和固定像素表面。
pub struct OverlayWindow {
    title: String,
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
}

impl OverlayWindow {
    pub fn open(
        event_loop: &ActiveEventLoop,
        context: &Context<Rc<Window>>,
        title: String,
        attributes: WindowAttributes,
        pixel_size: PixelSize,
    ) -> Result<Self, PinoraError> {
        let window = window_policy::create_auxiliary_window(
            event_loop,
            AuxiliaryWindowKind::Overlay,
            attributes,
        )
        .map_err(|error| {
            PinoraError::new(ErrorCode::Internal, format!("overlay window: {error}"))
        })?;
        let window = Rc::new(window);
        let mut surface = Surface::new(context, window.clone()).map_err(|error| {
            PinoraError::new(ErrorCode::Internal, format!("overlay surface: {error}"))
        })?;
        sync_surface_size(&mut surface, pixel_size);

        Ok(Self {
            title,
            window,
            surface,
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn inner_size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.window.inner_size()
    }

    pub fn set_ime_allowed(&self, allowed: bool) {
        self.window.set_ime_allowed(allowed);
    }

    pub fn show(&self) {
        window_policy::show_auxiliary_window(
            AuxiliaryWindowKind::Overlay,
            &self.window,
            &self.title,
        );
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn focus(&self) {
        self.window.focus_window();
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Overlay 以原始截图像素大小保持 Surface，窗口客户区 resize 不得触发整屏重采样。
    pub fn sync_pixel_size(&mut self, pixel_size: PixelSize) {
        sync_surface_size(&mut self.surface, pixel_size);
    }

    pub fn surface_mut(&mut self) -> &mut Surface<Rc<Window>, Rc<Window>> {
        &mut self.surface
    }
}

fn sync_surface_size(surface: &mut Surface<Rc<Window>, Rc<Window>>, pixel_size: PixelSize) {
    if let (Some(width), Some(height)) = (
        NonZeroU32::new(pixel_size.width.max(1)),
        NonZeroU32::new(pixel_size.height.max(1)),
    ) {
        let _ = surface.resize(width, height);
    }
}
