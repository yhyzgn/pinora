//! 设置窗口的 winit/softbuffer 适配器。
//!
//! 本模块持有窗口、设置草稿面板和既有原子存储句柄。调用方决定何时把成功保存的
//! 值应用到 runtime 与历史策略，避免 UI 适配器直接改变应用工作流。

use std::num::NonZeroU32;
use std::rc::Rc;

use pinora_core::{AppSettings, ErrorCode, HotkeyBinding, PinoraError, PixelPoint};
use softbuffer::{Context, Surface};
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

use pinora_desktop::panel_theme::{PanelThemeState, SystemAppearance};
use pinora_desktop::settings_panel::{
    self, SettingField, SettingsPanel, SettingsPanelAction, SettingsPanelKey,
};
use pinora_desktop::window_policy::{self, AuxiliaryWindowKind};
use pinora_storage::{SettingsStore, default_settings_path};

/// 单个设置窗口的资源、草稿和原子保存入口。
pub struct SettingsWindow {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    panel: SettingsPanel,
    theme: PanelThemeState,
    store: SettingsStore,
    cursor: PixelPoint,
    width: u32,
    height: u32,
}

impl SettingsWindow {
    pub fn open(
        event_loop: &ActiveEventLoop,
        context: &Context<Rc<Window>>,
        current: AppSettings,
    ) -> Result<Self, PinoraError> {
        let attrs = Window::default_attributes()
            .with_title("Pinora Settings")
            .with_inner_size(PhysicalSize::new(
                settings_panel::PANEL_WIDTH,
                settings_panel::PANEL_HEIGHT,
            ))
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(false);
        let window = window_policy::create_auxiliary_window(
            event_loop,
            AuxiliaryWindowKind::Panel,
            attrs,
        )
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("settings window: {e}")))?;
        let window = Rc::new(window);
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
        let settings = Self {
            theme: PanelThemeState::new(
                current.theme,
                SystemAppearance::from_winit(window.theme()),
            ),
            window,
            surface,
            panel: SettingsPanel::new(current),
            store: SettingsStore::new(default_settings_path()),
            cursor: PixelPoint::new(0, 0),
            width: settings_panel::PANEL_WIDTH,
            height: settings_panel::PANEL_HEIGHT,
        };
        window_policy::show_auxiliary_window(
            AuxiliaryWindowKind::Panel,
            &settings.window,
            "Pinora Settings",
        );
        Ok(settings)
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

    pub fn set_cursor(&mut self, cursor: PixelPoint) {
        self.cursor = cursor;
    }

    pub fn hit_test(&self) -> Option<SettingsPanelAction> {
        SettingsPanel::hit_test(self.cursor)
    }

    pub fn handle_key(&mut self, key: SettingsPanelKey) -> Option<SettingsPanelAction> {
        let action = self.panel.handle_key(key);
        self.theme.set_preference(self.panel.draft().theme);
        action
    }

    pub fn apply_action(&mut self, action: SettingsPanelAction) {
        self.panel.apply_action(action);
        self.theme.set_preference(self.panel.draft().theme);
    }

    pub fn start_hotkey_recording(&mut self) {
        self.panel.start_hotkey_recording();
    }

    pub fn recording_hotkey_field(&self) -> Option<SettingField> {
        self.panel.recording_field()
    }

    pub fn record_hotkey(&mut self, binding: HotkeyBinding) -> Result<(), &'static str> {
        self.panel.record_hotkey(binding)
    }

    pub fn reject_hotkey_recording(&mut self, code: &'static str) {
        self.panel.reject_hotkey_recording(code);
    }

    pub fn draft(&self) -> AppSettings {
        self.panel.draft()
    }

    pub fn save(&self, draft: AppSettings) -> Result<(), String> {
        self.store.save(draft)
    }

    pub fn mark_saved(&mut self) {
        self.panel.mark_saved();
    }

    pub fn mark_save_failed(&mut self, code: &'static str) {
        self.panel.mark_save_failed(code);
    }

    pub fn handle_system_theme_change(&mut self, theme: winit::window::Theme) {
        if self
            .theme
            .update_system_appearance(SystemAppearance::from_winit(Some(theme)))
        {
            self.request_redraw();
        }
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

    pub fn paint(&mut self) -> Result<(), PinoraError> {
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
        settings_panel::paint(
            &self.panel,
            self.theme.palette(),
            &mut buffer[..width * height],
            width,
            height,
        );
        buffer
            .present()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("settings present: {e}")))?;
        Ok(())
    }
}
