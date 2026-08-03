//! 诊断窗口的 winit/softbuffer 适配器。
//!
//! 诊断模型由调用方提供，本模块只持有短生命周期的辅助窗口和呈现资源；关闭后窗口
//! 立即释放，Pinora 回到 tray-only 空闲状态。

use std::num::NonZeroU32;
use std::rc::Rc;

use pinora_core::{ErrorCode, PinoraError, ThemeMode};
use softbuffer::{Context, Surface};
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

use crate::diagnostics_panel::{self, DiagnosticsPanel};
use crate::panel_theme::{PanelThemeState, SystemAppearance};
use crate::tray_feedback::TrayFeedback;
use pinora_desktop::window_policy::{self, AuxiliaryWindowKind};

/// 单个诊断窗口的资源与受控快照。
pub(crate) struct DiagnosticsWindow {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    panel: DiagnosticsPanel,
    theme: PanelThemeState,
    width: u32,
    height: u32,
}

impl DiagnosticsWindow {
    pub(crate) fn open(
        event_loop: &ActiveEventLoop,
        context: &Context<Rc<Window>>,
        panel: DiagnosticsPanel,
        theme_preference: ThemeMode,
    ) -> Result<Self, PinoraError> {
        let attrs = Window::default_attributes()
            .with_title("Pinora Diagnostics")
            .with_inner_size(PhysicalSize::new(
                diagnostics_panel::PANEL_WIDTH,
                diagnostics_panel::PANEL_HEIGHT,
            ))
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(false);
        let window =
            window_policy::create_auxiliary_window(event_loop, AuxiliaryWindowKind::Panel, attrs)
                .map_err(|error| {
                PinoraError::new(ErrorCode::Internal, format!("diagnostics window: {error}"))
            })?;
        let window = Rc::new(window);
        let mut surface = Surface::new(context, window.clone()).map_err(|error| {
            PinoraError::new(ErrorCode::Internal, format!("diagnostics surface: {error}"))
        })?;
        if let (Some(width), Some(height)) = (
            NonZeroU32::new(diagnostics_panel::PANEL_WIDTH),
            NonZeroU32::new(diagnostics_panel::PANEL_HEIGHT),
        ) {
            surface.resize(width, height).map_err(|error| {
                PinoraError::new(ErrorCode::Internal, format!("diagnostics resize: {error}"))
            })?;
        }
        let diagnostics = Self {
            theme: PanelThemeState::new(
                theme_preference,
                SystemAppearance::from_winit(window.theme()),
            ),
            window,
            surface,
            panel,
            width: diagnostics_panel::PANEL_WIDTH,
            height: diagnostics_panel::PANEL_HEIGHT,
        };
        window_policy::show_auxiliary_window(
            AuxiliaryWindowKind::Panel,
            &diagnostics.window,
            "Pinora Diagnostics",
        );
        Ok(diagnostics)
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

    pub(crate) fn set_panel(&mut self, panel: DiagnosticsPanel) {
        self.panel = panel;
        self.request_redraw();
    }

    pub(crate) fn set_feedback(&mut self, feedback: TrayFeedback) {
        self.panel.set_feedback(feedback);
        self.request_redraw();
    }

    pub(crate) fn set_theme_preference(&mut self, preference: ThemeMode) {
        if self.theme.set_preference(preference) {
            self.request_redraw();
        }
    }

    pub(crate) fn handle_system_theme_change(&mut self, theme: winit::window::Theme) {
        if self
            .theme
            .update_system_appearance(SystemAppearance::from_winit(Some(theme)))
        {
            self.request_redraw();
        }
    }

    pub(crate) fn request_redraw(&self) {
        self.window.request_redraw();
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
            self.surface.resize(width, height).map_err(|error| {
                PinoraError::new(
                    ErrorCode::Internal,
                    format!("diagnostics surface resize: {error}"),
                )
            })?;
        }
        let mut buffer = self.surface.buffer_mut().map_err(|error| {
            PinoraError::new(ErrorCode::Internal, format!("diagnostics buffer: {error}"))
        })?;
        let width = self.width as usize;
        let height = self.height as usize;
        if buffer.len() < width.saturating_mul(height) {
            return Ok(());
        }
        diagnostics_panel::paint(
            &self.panel,
            self.theme.palette(),
            &mut buffer[..width * height],
            width,
            height,
        );
        buffer.present().map_err(|error| {
            PinoraError::new(ErrorCode::Internal, format!("diagnostics present: {error}"))
        })?;
        Ok(())
    }
}
