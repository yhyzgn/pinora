//! 设置窗口的 winit/softbuffer 适配器。
//!
//! 本模块持有窗口、设置草稿面板和既有原子存储句柄。调用方决定何时把成功保存的
//! 值应用到 runtime 与历史策略，避免 UI 适配器直接改变应用工作流。

use std::num::NonZeroU32;
use std::rc::Rc;

use pinora_core::{AppSettings, ErrorCode, PinoraError, PixelPoint};
use softbuffer::{Context, Surface};
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

use crate::settings_panel::{self, SettingsPanel, SettingsPanelAction, SettingsPanelKey};
use crate::settings_store::{SettingsStore, default_settings_path};
use crate::window_policy::{self, AuxiliaryWindowKind};

/// 单个设置窗口的资源、草稿和原子保存入口。
pub(crate) struct SettingsWindow {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    panel: SettingsPanel,
    store: SettingsStore,
    cursor: PixelPoint,
    width: u32,
    height: u32,
}

impl SettingsWindow {
    pub(crate) fn open(
        event_loop: &ActiveEventLoop,
        context: &Context<Rc<Window>>,
        current: AppSettings,
    ) -> Result<Self, PinoraError> {
        let attrs = window_policy::auxiliary_window_attributes(
            AuxiliaryWindowKind::Panel,
            Window::default_attributes()
                .with_title("Pinora Settings")
                .with_inner_size(PhysicalSize::new(
                    settings_panel::PANEL_WIDTH,
                    settings_panel::PANEL_HEIGHT,
                ))
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_visible(true),
        );
        let window = event_loop
            .create_window(attrs)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("settings window: {e}")))?;
        let window = Rc::new(window);
        if crate::kwin_place::kwin_available() {
            crate::kwin_place::mark_auxiliary_window_by_title("Pinora Settings", 50);
        }
        let mut surface = Surface::new(context, window.clone())
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("settings surface: {e}")))?;
        if let (Some(width), Some(height)) = (
            NonZeroU32::new(settings_panel::PANEL_WIDTH),
            NonZeroU32::new(settings_panel::PANEL_HEIGHT),
        ) {
            surface.resize(width, height).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("settings resize: {e}"))
            })?;
        }
        Ok(Self {
            window,
            surface,
            panel: SettingsPanel::new(current),
            store: SettingsStore::new(default_settings_path()),
            cursor: PixelPoint::new(0, 0),
            width: settings_panel::PANEL_WIDTH,
            height: settings_panel::PANEL_HEIGHT,
        })
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub(crate) fn focus(&self) {
        self.window.focus_window();
    }

    pub(crate) fn close(self) {
        self.window.set_visible(false);
    }

    pub(crate) fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub(crate) fn set_cursor(&mut self, cursor: PixelPoint) {
        self.cursor = cursor;
    }

    pub(crate) fn hit_test(&self) -> Option<SettingsPanelAction> {
        SettingsPanel::hit_test(self.cursor)
    }

    pub(crate) fn handle_key(&mut self, key: SettingsPanelKey) -> Option<SettingsPanelAction> {
        self.panel.handle_key(key)
    }

    pub(crate) fn apply_action(&mut self, action: SettingsPanelAction) {
        self.panel.apply_action(action);
    }

    pub(crate) fn draft(&self) -> AppSettings {
        self.panel.draft()
    }

    pub(crate) fn save(&self, draft: AppSettings) -> Result<(), String> {
        self.store.save(draft)
    }

    pub(crate) fn mark_saved(&mut self) {
        self.panel.mark_saved();
    }

    pub(crate) fn mark_save_failed(&mut self, code: &'static str) {
        self.panel.mark_save_failed(code);
    }

    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(self.width), NonZeroU32::new(self.height))
        {
            let _ = self.surface.resize(width, height);
        }
        self.request_redraw();
    }

    pub(crate) fn paint(&mut self) -> Result<(), PinoraError> {
        let size = self.window.inner_size();
        self.width = size.width.max(1);
        self.height = size.height.max(1);
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(self.width), NonZeroU32::new(self.height))
        {
            self.surface.resize(width, height).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("settings surface resize: {e}"))
            })?;
        }
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("settings buffer: {e}")))?;
        let width = self.width as usize;
        let height = self.height as usize;
        if buffer.len() < width.saturating_mul(height) {
            return Ok(());
        }
        settings_panel::paint(&self.panel, &mut buffer[..width * height], width, height);
        buffer
            .present()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("settings present: {e}")))?;
        Ok(())
    }
}
