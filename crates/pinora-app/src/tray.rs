//! 系统托盘（Linux StatusNotifier / appindicator via tray-icon）。
//!
//! 注意：Linux 后端基于 GTK，创建托盘前必须 `gtk::init()`，否则会直接 panic。

use std::panic::{catch_unwind, AssertUnwindSafe};

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// 托盘菜单动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Capture,
    Quit,
}

/// 托盘句柄：持有 TrayIcon 与菜单项 id。
pub struct AppTray {
    _tray: TrayIcon,
    capture_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
}

impl AppTray {
    /// 创建托盘图标。失败时返回错误（无托盘环境不致命，由调用方降级）。
    pub fn try_new() -> Result<Self, String> {
        // tray-icon 内部可能 panic（未 gtk::init），必须 catch
        match catch_unwind(AssertUnwindSafe(try_new_inner)) {
            Ok(r) => r,
            Err(_) => Err(
                "tray panicked (often GTK not initialized or no display)".into(),
            ),
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
            if ev.id == self.quit_id {
                return Some(TrayAction::Quit);
            }
        }
        None
    }
}

fn try_new_inner() -> Result<AppTray, String> {
    // Linux tray-icon → appindicator → GTK 菜单
    if gtk::init().is_err() {
        // 可能已初始化
        if !gtk::is_initialized() {
            return Err("gtk::init failed".into());
        }
    }

    let icon = make_icon().map_err(|e| format!("tray icon: {e}"))?;
    let menu = Menu::new();
    let capture = MenuItem::new("截图 (F2)", true, None);
    let quit = MenuItem::new("退出", true, None);
    menu.append(&capture)
        .map_err(|e| format!("menu append capture: {e}"))?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| format!("menu sep: {e}"))?;
    menu.append(&quit)
        .map_err(|e| format!("menu append quit: {e}"))?;

    let capture_id = capture.id().clone();
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
        quit_id,
    })
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
