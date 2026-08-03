//! 基于 xcap 的真实屏幕捕获。

use std::time::{SystemTime, UNIX_EPOCH};

use pinora_core::{
    CaptureImage, CaptureMetadata, CaptureProvider, CaptureRequest, CaptureWindowId,
    CaptureWindowInfo, DisplayId, DisplayInfo, ErrorCode, ImageId, PinoraError, PixelRect,
    PixelSize, RgbaBuffer, resolve_capture_rect,
};
use xcap::{Monitor, Window};

/// xcap 实现的捕获提供者。
#[derive(Debug, Default, Clone, Copy)]
pub struct XcapCaptureProvider;

impl XcapCaptureProvider {
    pub fn new() -> Self {
        Self
    }

    /// 探测能否枚举至少一个显示器（不执行像素捕获）。
    pub fn probe_available() -> Result<Vec<DisplayInfo>, PinoraError> {
        Self::new().displays()
    }
}

impl CaptureProvider for XcapCaptureProvider {
    fn displays(&self) -> Result<Vec<DisplayInfo>, PinoraError> {
        let monitors = Monitor::all().map_err(map_xcap)?;
        if monitors.is_empty() {
            return Err(PinoraError::new(
                ErrorCode::CapabilityUnavailable,
                "xcap: no monitors found",
            ));
        }
        monitors.iter().map(monitor_to_display).collect()
    }

    fn windows(&self) -> Result<Vec<CaptureWindowInfo>, PinoraError> {
        let displays = self.displays()?;
        Window::all()
            .map_err(map_xcap)?
            .iter()
            .filter_map(|window| match window_to_capture_info(window, &displays) {
                Ok(Some(info)) => Some(Ok(info)),
                Ok(None) => None,
                // 一个系统窗口的元数据损坏、权限受限或瞬时消失不应让整个 tray 菜单
                // 不可用；只有已经完整验证的候选才会暴露给用户。
                Err(_) => None,
            })
            .collect()
    }

    fn capture(&self, request: CaptureRequest) -> Result<CaptureImage, PinoraError> {
        if matches!(request, CaptureRequest::AllDisplays) {
            return Err(PinoraError::new(
                ErrorCode::CapabilityUnavailable,
                "xcap does not provide a single atomic all-displays capture",
            ));
        }
        if let CaptureRequest::Window { target } = request {
            return self.capture_window(target);
        }
        let displays = self.displays()?;
        let (info, rect) = resolve_capture_rect(&displays, &request)?;
        let monitor = find_monitor_for_display(&info.id)?;

        let local_x = (rect.origin.x - info.bounds.origin.x).max(0) as u32;
        let local_y = (rect.origin.y - info.bounds.origin.y).max(0) as u32;
        let width = rect.size.width;
        let height = rect.size.height;

        let rgba = if local_x == 0
            && local_y == 0
            && width == info.bounds.size.width
            && height == info.bounds.size.height
        {
            monitor.capture_image().map_err(map_xcap)?
        } else {
            monitor
                .capture_region(local_x, local_y, width, height)
                .map_err(map_xcap)?
        };

        let (w, h) = rgba.dimensions();
        if w != width || h != height {
            // 部分平台可能返回不同尺寸；以实际缓冲为准。
        }
        let bytes = rgba.into_raw();
        let size = PixelSize::new(w, h);
        let pixels = RgbaBuffer::new(size, bytes)
            .map_err(|msg| PinoraError::new(ErrorCode::Internal, format!("xcap buffer: {msg}")))?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let source_rect = PixelRect::new(rect.origin.x, rect.origin.y, w, h);
        CaptureImage::new(
            ImageId::new(),
            pixels,
            source_rect,
            CaptureMetadata::new(info.id, info.scale, now_ms),
        )
        .map_err(|msg| PinoraError::new(ErrorCode::Internal, msg))
    }
}

impl XcapCaptureProvider {
    fn capture_window(&self, target: CaptureWindowInfo) -> Result<CaptureImage, PinoraError> {
        let displays = self.displays()?;
        let (window, current) = find_window_for_target(&target, &displays)?;
        if !target.matches_capture_snapshot(&current) {
            return Err(PinoraError::new(
                ErrorCode::NotFound,
                "capture window no longer matches the menu snapshot",
            ));
        }

        let rgba = window.capture_image().map_err(map_xcap)?;
        let (width, height) = rgba.dimensions();
        if width == 0 || height == 0 {
            return Err(PinoraError::new(
                ErrorCode::RetryablePlatform,
                "window capture returned an empty image",
            ));
        }
        if width != current.bounds.size.width || height != current.bounds.size.height {
            return Err(PinoraError::new(
                ErrorCode::RetryablePlatform,
                "window capture dimensions changed while capturing",
            ));
        }
        let size = PixelSize::new(width, height);
        let pixels = RgbaBuffer::new(size, rgba.into_raw())
            .map_err(|message| PinoraError::new(ErrorCode::Internal, message))?;
        CaptureImage::new(
            ImageId::new(),
            pixels,
            PixelRect::new(
                current.bounds.origin.x,
                current.bounds.origin.y,
                width,
                height,
            ),
            CaptureMetadata::new(current.display, current.scale, now_ms()),
        )
        .map_err(|message| PinoraError::new(ErrorCode::Internal, message))
    }
}

fn monitor_to_display(monitor: &Monitor) -> Result<DisplayInfo, PinoraError> {
    let id = monitor.id().map_err(map_xcap)?;
    let name = monitor
        .friendly_name()
        .or_else(|_| monitor.name())
        .unwrap_or_else(|_| format!("Monitor-{id}"));
    let x = monitor.x().map_err(map_xcap)?;
    let y = monitor.y().map_err(map_xcap)?;
    let width = monitor.width().map_err(map_xcap)?;
    let height = monitor.height().map_err(map_xcap)?;
    let scale = f64::from(monitor.scale_factor().unwrap_or(1.0));
    Ok(DisplayInfo {
        id: DisplayId::new(format!("xcap-{id}")),
        name,
        bounds: PixelRect::new(x, y, width, height),
        scale: if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        },
    })
}

fn find_monitor_for_display(display_id: &DisplayId) -> Result<Monitor, PinoraError> {
    let monitors = Monitor::all().map_err(map_xcap)?;
    for monitor in monitors {
        let info = monitor_to_display(&monitor)?;
        if &info.id == display_id {
            return Ok(monitor);
        }
    }
    Err(PinoraError::new(
        ErrorCode::NotFound,
        format!("xcap monitor not found: {}", display_id.0),
    ))
}

fn find_window_for_target(
    target: &CaptureWindowInfo,
    displays: &[DisplayInfo],
) -> Result<(Window, CaptureWindowInfo), PinoraError> {
    for window in Window::all().map_err(map_xcap)? {
        let Some(current) = window_to_capture_info(&window, displays)? else {
            continue;
        };
        if current.id == target.id {
            return Ok((window, current));
        }
    }
    Err(PinoraError::new(
        ErrorCode::NotFound,
        "capture window is no longer available",
    ))
}

fn window_to_capture_info(
    window: &Window,
    displays: &[DisplayInfo],
) -> Result<Option<CaptureWindowInfo>, PinoraError> {
    let app_name = window.app_name().map_err(map_xcap)?;
    let title = window.title().map_err(map_xcap)?;
    let is_minimized = window.is_minimized().map_err(map_xcap)?;
    let width = window.width().map_err(map_xcap)?;
    let height = window.height().map_err(map_xcap)?;
    let bounds = PixelRect::new(
        window.x().map_err(map_xcap)?,
        window.y().map_err(map_xcap)?,
        width,
        height,
    );
    if !is_capturable_window(&app_name, &title, bounds, is_minimized) {
        return Ok(None);
    }

    let monitor = window.current_monitor().map_err(map_xcap)?;
    let monitor_id = monitor.id().map_err(map_xcap)?;
    let display_id = DisplayId::new(format!("xcap-{monitor_id}"));
    let display = displays
        .iter()
        .find(|display| display.id == display_id)
        .ok_or_else(|| {
            PinoraError::new(
                ErrorCode::NotFound,
                "capture window display is no longer available",
            )
        })?;

    Ok(Some(CaptureWindowInfo {
        id: CaptureWindowId::from_raw(u64::from(window.id().map_err(map_xcap)?)),
        app_name,
        title,
        bounds,
        display: display.id.clone(),
        scale: display.scale,
        is_minimized,
    }))
}

fn is_capturable_window(
    app_name: &str,
    title: &str,
    bounds: PixelRect,
    is_minimized: bool,
) -> bool {
    !is_minimized
        && !bounds.size.is_empty()
        && !is_pinora_window(app_name, title)
        && (!app_name.trim().is_empty() || !title.trim().is_empty())
}

fn is_pinora_window(app_name: &str, title: &str) -> bool {
    app_name.trim().eq_ignore_ascii_case("pinora")
        || matches!(
            title.trim(),
            "pinora-display-handle"
                | "Pinora Settings"
                | "Pinora History"
                | "Pinora History Edit"
                | "Pinora Window Capture"
                | "Pinora Virtual Desktop Capture"
                | "Pinora Pin Edit"
        )
        || title.starts_with("Pinora-pin-")
        || title.starts_with("Pinora —")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn map_xcap(err: xcap::XCapError) -> PinoraError {
    let msg = err.to_string();
    let lower = msg.to_ascii_lowercase();
    let code = if lower.contains("permission")
        || lower.contains("denied")
        || lower.contains("portal")
        || lower.contains("not authorized")
    {
        ErrorCode::PermissionDenied
    } else if lower.contains("not support") || lower.contains("unavailable") {
        ErrorCode::CapabilityUnavailable
    } else {
        ErrorCode::RetryablePlatform
    };
    PinoraError::new(code, format!("xcap: {msg}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires display session and screen capture permission"]
    fn real_capture_primary_monitor() {
        let provider = XcapCaptureProvider::new();
        let displays = provider.displays().expect("displays");
        assert!(!displays.is_empty());
        let d0 = &displays[0];
        let image = provider
            .capture(CaptureRequest::Region {
                display: d0.id.clone(),
                rect: PixelRect::new(
                    d0.bounds.origin.x,
                    d0.bounds.origin.y,
                    64.min(d0.bounds.size.width),
                    64.min(d0.bounds.size.height),
                ),
            })
            .expect("capture");
        assert!(image.size().width > 0);
        assert!(image.size().height > 0);
        assert_eq!(image.pixels.byte_len(), (image.size().area() * 4) as usize);
    }

    #[test]
    fn candidate_filter_excludes_pinora_minimized_empty_and_unnamed_windows() {
        let bounds = PixelRect::new(10, 20, 100, 50);

        assert!(!is_capturable_window("Pinora", "Other", bounds, false));
        assert!(!is_capturable_window(
            "Other",
            "Pinora-pin-pin-1",
            bounds,
            false
        ));
        assert!(!is_capturable_window(
            "Other",
            "Pinora Window Capture",
            bounds,
            false
        ));
        assert!(!is_capturable_window("Other", "Other", bounds, true));
        assert!(!is_capturable_window(
            "Other",
            "Other",
            PixelRect::new(0, 0, 0, 50),
            false
        ));
        assert!(!is_capturable_window("", "", bounds, false));
        assert!(is_capturable_window(
            "Browser",
            "Documentation",
            bounds,
            false
        ));
    }

    #[test]
    fn all_displays_is_rejected_instead_of_stitching_multiple_frames() {
        let error = XcapCaptureProvider::new()
            .capture(CaptureRequest::AllDisplays)
            .expect_err("xcap cannot promise one atomic workspace frame");
        assert_eq!(error.code, ErrorCode::CapabilityUnavailable);
    }
}
