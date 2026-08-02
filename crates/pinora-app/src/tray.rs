//! 系统托盘（通过 `tray-icon` 适配各平台）。
//!
//! Linux 的 StatusNotifier/appindicator 后端依赖 GTK；GTK 只在 Linux target
//! 编译并在创建托盘前初始化，Windows/macOS 走 tray-icon 原生后端。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use pinora_core::{DisplayId, DisplayInfo};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// 托盘菜单动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    Capture,
    CaptureRegionAfter(Duration),
    CancelDelayedCapture,
    CaptureFullDisplay,
    CaptureDisplay(DisplayId),
    Settings,
    History,
    ShowAllPins,
    HideAllPins,
    CloseAllPins,
    Quit,
}

/// 托盘句柄：持有 TrayIcon 与菜单项 id。
pub struct AppTray {
    _tray: TrayIcon,
    capture_id: tray_icon::menu::MenuId,
    delay_capture_ids: [(MenuId, Duration); 3],
    cancel_delayed_capture_id: tray_icon::menu::MenuId,
    delay_capture_items: [MenuItem; 3],
    cancel_delayed_capture_item: MenuItem,
    capture_full_display_id: tray_icon::menu::MenuId,
    capture_display_ids: Vec<(MenuId, DisplayId)>,
    settings_id: tray_icon::menu::MenuId,
    history_id: tray_icon::menu::MenuId,
    show_all_pins_id: tray_icon::menu::MenuId,
    hide_all_pins_id: tray_icon::menu::MenuId,
    close_all_pins_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
}

impl AppTray {
    /// 创建托盘图标；调用方决定失败是否允许启动。
    pub fn try_new(displays: &[DisplayInfo]) -> Result<Self, String> {
        // tray-icon 内部可能 panic（未 gtk::init），必须 catch
        match catch_unwind(AssertUnwindSafe(|| try_new_inner(displays))) {
            Ok(r) => r,
            Err(_) => Err("tray panicked (often GTK not initialized or no display)".into()),
        }
    }

    /// 非阻塞轮询菜单动作。
    pub fn poll(&self) -> Option<TrayAction> {
        while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
            if matches!(
                ev,
                TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                }
            ) {
                return Some(TrayAction::Capture);
            }
        }
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == self.capture_id {
                return Some(TrayAction::Capture);
            }
            if let Some(action) = delayed_capture_action(&ev.id, &self.delay_capture_ids) {
                return Some(action);
            }
            if ev.id == self.cancel_delayed_capture_id {
                return Some(TrayAction::CancelDelayedCapture);
            }
            if ev.id == self.capture_full_display_id {
                return Some(TrayAction::CaptureFullDisplay);
            }
            if let Some(action) = display_capture_action(&ev.id, &self.capture_display_ids) {
                return Some(action);
            }
            if ev.id == self.settings_id {
                return Some(TrayAction::Settings);
            }
            if ev.id == self.history_id {
                return Some(TrayAction::History);
            }
            if ev.id == self.show_all_pins_id {
                return Some(TrayAction::ShowAllPins);
            }
            if ev.id == self.hide_all_pins_id {
                return Some(TrayAction::HideAllPins);
            }
            if ev.id == self.close_all_pins_id {
                return Some(TrayAction::CloseAllPins);
            }
            if ev.id == self.quit_id {
                return Some(TrayAction::Quit);
            }
        }
        None
    }

    /// 延时会话期间只允许取消，避免重复倒计时或在隐藏贴图期间切换截图流程。
    pub fn set_delayed_capture_active(&self, active: bool) {
        for item in &self.delay_capture_items {
            item.set_enabled(!active);
        }
        self.cancel_delayed_capture_item.set_enabled(active);
    }
}

fn try_new_inner(displays: &[DisplayInfo]) -> Result<AppTray, String> {
    // Linux tray-icon → appindicator → GTK 菜单；GTK 不应进入其他 target。
    #[cfg(target_os = "linux")]
    if gtk::init().is_err() {
        // 可能已初始化
        if !gtk::is_initialized() {
            return Err("gtk::init failed".into());
        }
    }

    let icon = make_icon().map_err(|e| format!("tray icon: {e}"))?;
    let menu = Menu::new();
    let capture = MenuItem::new("截图 (F2)", true, None);
    let delay_capture_one = MenuItem::new("延时截图 1 秒", true, None);
    let delay_capture_three = MenuItem::new("延时截图 3 秒", true, None);
    let delay_capture_five = MenuItem::new("延时截图 5 秒", true, None);
    let cancel_delayed_capture = MenuItem::new("取消延时截图", false, None);
    let capture_full_display = MenuItem::new("全屏截图 (F3)", true, None);
    let mut capture_display_ids = Vec::new();
    let settings = MenuItem::new("设置", true, None);
    let history = MenuItem::new("历史", true, None);
    let show_all_pins = MenuItem::new("显示全部贴图", true, None);
    let hide_all_pins = MenuItem::new("隐藏全部贴图", true, None);
    let close_all_pins = MenuItem::new("关闭全部贴图", true, None);
    let quit = MenuItem::new("退出", true, None);
    menu.append(&capture)
        .map_err(|e| format!("menu append capture: {e}"))?;
    menu.append(&delay_capture_one)
        .map_err(|e| format!("menu append delayed capture 1: {e}"))?;
    menu.append(&delay_capture_three)
        .map_err(|e| format!("menu append delayed capture 3: {e}"))?;
    menu.append(&delay_capture_five)
        .map_err(|e| format!("menu append delayed capture 5: {e}"))?;
    menu.append(&cancel_delayed_capture)
        .map_err(|e| format!("menu append cancel delayed capture: {e}"))?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| format!("menu sep delayed capture: {e}"))?;
    menu.append(&capture_full_display)
        .map_err(|e| format!("menu append full-display capture: {e}"))?;
    if displays.len() > 1 {
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| format!("menu sep displays: {e}"))?;
        for display in displays {
            let capture_display = MenuItem::new(display_capture_label(display), true, None);
            menu.append(&capture_display)
                .map_err(|e| format!("menu append display capture: {e}"))?;
            capture_display_ids.push((capture_display.id().clone(), display.id.clone()));
        }
    }
    menu.append(&settings)
        .map_err(|e| format!("menu append settings: {e}"))?;
    menu.append(&history)
        .map_err(|e| format!("menu append history: {e}"))?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| format!("menu sep pins: {e}"))?;
    menu.append(&show_all_pins)
        .map_err(|e| format!("menu append show pins: {e}"))?;
    menu.append(&hide_all_pins)
        .map_err(|e| format!("menu append hide pins: {e}"))?;
    menu.append(&close_all_pins)
        .map_err(|e| format!("menu append close pins: {e}"))?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| format!("menu sep: {e}"))?;
    menu.append(&quit)
        .map_err(|e| format!("menu append quit: {e}"))?;

    let capture_id = capture.id().clone();
    let delay_capture_ids = [
        (delay_capture_one.id().clone(), Duration::from_secs(1)),
        (delay_capture_three.id().clone(), Duration::from_secs(3)),
        (delay_capture_five.id().clone(), Duration::from_secs(5)),
    ];
    let cancel_delayed_capture_id = cancel_delayed_capture.id().clone();
    let capture_full_display_id = capture_full_display.id().clone();
    let settings_id = settings.id().clone();
    let history_id = history.id().clone();
    let show_all_pins_id = show_all_pins.id().clone();
    let hide_all_pins_id = hide_all_pins.id().clone();
    let close_all_pins_id = close_all_pins.id().clone();
    let quit_id = quit.id().clone();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Pinora — 截图 / 贴图")
        .with_icon(icon)
        .build()
        .map_err(|e| format!("tray build: {e}"))?;

    Ok(AppTray {
        _tray: tray,
        capture_id,
        delay_capture_ids,
        cancel_delayed_capture_id,
        delay_capture_items: [delay_capture_one, delay_capture_three, delay_capture_five],
        cancel_delayed_capture_item: cancel_delayed_capture,
        capture_full_display_id,
        capture_display_ids,
        settings_id,
        history_id,
        show_all_pins_id,
        hide_all_pins_id,
        close_all_pins_id,
        quit_id,
    })
}

fn display_capture_label(display: &DisplayInfo) -> String {
    format!(
        "全屏截图：{} [{}, {}] {}x{} @ {:.2}x",
        display.name,
        display.bounds.origin.x,
        display.bounds.origin.y,
        display.bounds.size.width,
        display.bounds.size.height,
        display.scale
    )
}

fn display_capture_action(
    menu_id: &MenuId,
    display_ids: &[(MenuId, DisplayId)],
) -> Option<TrayAction> {
    display_ids
        .iter()
        .find(|(id, _)| id == menu_id)
        .map(|(_, display_id)| TrayAction::CaptureDisplay(display_id.clone()))
}

fn delayed_capture_action(
    menu_id: &MenuId,
    delay_capture_ids: &[(MenuId, Duration); 3],
) -> Option<TrayAction> {
    delay_capture_ids
        .iter()
        .find(|(id, _)| id == menu_id)
        .map(|(_, delay)| TrayAction::CaptureRegionAfter(*delay))
}

fn make_icon() -> Result<Icon, String> {
    // 32x32 简单蓝底白十字
    let w = 32u32;
    let h = 32u32;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let cx = x as i32 - 16;
            let cy = y as i32 - 16;
            let in_cross = cx.abs() <= 2 || cy.abs() <= 2;
            let in_border = x < 2 || y < 2 || x >= w - 2 || y >= h - 2;
            if in_border {
                rgba[i] = 30;
                rgba[i + 1] = 80;
                rgba[i + 2] = 180;
                rgba[i + 3] = 255;
            } else if in_cross && cx.abs() < 10 && cy.abs() < 10 {
                rgba[i] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            } else {
                rgba[i] = 40;
                rgba[i + 1] = 120;
                rgba[i + 2] = 220;
                rgba[i + 3] = 255;
            }
        }
    }
    Icon::from_rgba(rgba, w, h).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::PixelRect;

    fn display(id: &str, name: &str, x: i32, y: i32) -> DisplayInfo {
        DisplayInfo {
            id: DisplayId::new(id),
            name: name.into(),
            bounds: PixelRect::new(x, y, 2560, 1440),
            scale: 1.25,
        }
    }

    #[test]
    fn per_display_action_preserves_the_selected_display_id() {
        let menu_id = MenuId::new("capture-display-two");
        let displays = vec![(menu_id.clone(), DisplayId::new("display-two"))];

        assert_eq!(
            display_capture_action(&menu_id, &displays),
            Some(TrayAction::CaptureDisplay(DisplayId::new("display-two")))
        );
        assert!(display_capture_action(&MenuId::new("other"), &displays).is_none());
    }

    #[test]
    fn per_display_label_exposes_physical_topology_without_leaking_internal_id() {
        let label = display_capture_label(&display("private-backend-id", "Left", -2560, 0));

        assert!(label.contains("Left"));
        assert!(label.contains("-2560"));
        assert!(label.contains("2560x1440"));
        assert!(label.contains("1.25x"));
        assert!(!label.contains("private-backend-id"));
    }

    #[test]
    fn delayed_capture_action_preserves_the_requested_duration() {
        let menu_id = MenuId::new("delay-three");
        let delays = [
            (MenuId::new("delay-one"), Duration::from_secs(1)),
            (menu_id.clone(), Duration::from_secs(3)),
            (MenuId::new("delay-five"), Duration::from_secs(5)),
        ];

        assert_eq!(
            delayed_capture_action(&menu_id, &delays),
            Some(TrayAction::CaptureRegionAfter(Duration::from_secs(3)))
        );
        assert!(delayed_capture_action(&MenuId::new("other"), &delays).is_none());
    }
}
