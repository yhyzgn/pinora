//! 历史窗口的 winit/softbuffer 适配器。
//!
//! 历史索引、文件校验和用户动作仍由 `desktop_shell` 编排；本模块只拥有窗口、
//! 预览缓存、尺寸同步和帧呈现，避免这些平台资源回流到领域或文件事务层。

use std::num::NonZeroU32;
use std::rc::Rc;

use pinora_core::{
    ErrorCode, HistoryEntry, ImageId, PinoraError, PixelPoint, PixelSize, ThemeMode,
};
use softbuffer::{Context, Surface};
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

use pinora_desktop::history_browser::{self, HistoryPanel, HistoryPreview};
use pinora_desktop::panel_theme::{PanelThemeState, SystemAppearance};
use pinora_desktop::window_policy::{self, AuxiliaryWindowKind};

struct HistoryPreviewCache {
    entry_image_id: ImageId,
    pixels_xrgb: Vec<u32>,
    size: PixelSize,
}

/// 单个历史窗口的资源和呈现状态。
pub struct HistoryWindow {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    panel: HistoryPanel,
    theme: PanelThemeState,
    cursor: PixelPoint,
    width: u32,
    height: u32,
    preview: Option<HistoryPreviewCache>,
}

impl HistoryWindow {
    pub fn open(
        event_loop: &ActiveEventLoop,
        context: &Context<Rc<Window>>,
        entries: Vec<HistoryEntry>,
        theme_preference: ThemeMode,
    ) -> Result<Self, PinoraError> {
        let attrs = Window::default_attributes()
            .with_title("Pinora History")
            .with_inner_size(PhysicalSize::new(
                history_browser::PANEL_WIDTH,
                history_browser::PANEL_HEIGHT,
            ))
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(false);
        let window = window_policy::create_auxiliary_window(
            event_loop,
            AuxiliaryWindowKind::Panel,
            attrs,
        )
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("history window: {e}")))?;
        let window = Rc::new(window);
        let mut surface = Surface::new(context, window.clone())
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("history surface: {e}")))?;
        if let (Some(width), Some(height)) = (
            NonZeroU32::new(history_browser::PANEL_WIDTH),
            NonZeroU32::new(history_browser::PANEL_HEIGHT),
        ) {
            surface.resize(width, height).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("history resize: {e}"))
            })?;
        }
        let history = Self {
            theme: PanelThemeState::new(
                theme_preference,
                SystemAppearance::from_winit(window.theme()),
            ),
            window,
            surface,
            panel: HistoryPanel::new(entries),
            cursor: PixelPoint::new(0, 0),
            width: history_browser::PANEL_WIDTH,
            height: history_browser::PANEL_HEIGHT,
            preview: None,
        };
        window_policy::show_auxiliary_window(
            AuxiliaryWindowKind::Panel,
            &history.window,
            "Pinora History",
        );
        Ok(history)
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn focus(&self) {
        self.window.focus_window();
    }

    pub fn close(self) {
        self.window.set_visible(false);
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn set_theme_preference(&mut self, preference: ThemeMode) {
        if self.theme.set_preference(preference) {
            self.request_redraw();
        }
    }

    pub fn handle_system_theme_change(&mut self, theme: winit::window::Theme) {
        if self
            .theme
            .update_system_appearance(SystemAppearance::from_winit(Some(theme)))
        {
            self.request_redraw();
        }
    }

    pub fn cursor(&self) -> PixelPoint {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: PixelPoint) {
        self.cursor = cursor;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(self.width), NonZeroU32::new(self.height))
        {
            let _ = self.surface.resize(width, height);
        }
        self.request_redraw();
    }

    pub fn panel(&self) -> &HistoryPanel {
        &self.panel
    }

    pub fn panel_mut(&mut self) -> &mut HistoryPanel {
        &mut self.panel
    }

    pub fn clear_preview(&mut self) {
        self.preview = None;
    }

    pub fn cache_preview(&mut self, image_id: ImageId, pixels_xrgb: Vec<u32>, size: PixelSize) {
        self.preview = Some(HistoryPreviewCache {
            entry_image_id: image_id,
            pixels_xrgb,
            size,
        });
    }

    pub fn paint(&mut self) -> Result<(), PinoraError> {
        let size = self.window.inner_size();
        self.width = size.width.max(1);
        self.height = size.height.max(1);
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(self.width), NonZeroU32::new(self.height))
        {
            self.surface.resize(width, height).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("history surface resize: {e}"))
            })?;
        }
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("history buffer: {e}")))?;
        let width = self.width as usize;
        let height = self.height as usize;
        if buffer.len() < width.saturating_mul(height) {
            return Ok(());
        }
        let selected_image_id = self.panel.selected_entry().map(|entry| entry.image_id);
        let preview = self.preview.as_ref().and_then(|preview| {
            (Some(preview.entry_image_id) == selected_image_id).then_some(HistoryPreview {
                pixels_xrgb: &preview.pixels_xrgb,
                size: preview.size,
            })
        });
        history_browser::paint(
            &self.panel,
            preview,
            self.theme.palette(),
            &mut buffer[..width * height],
            width,
            height,
        );
        buffer
            .present()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("history present: {e}")))?;
        Ok(())
    }
}
