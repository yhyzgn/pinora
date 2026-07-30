//! 基于 xcap 的真实屏幕捕获。

use std::time::{SystemTime, UNIX_EPOCH};

use pinora_core::{
    resolve_capture_rect, CaptureImage, CaptureMetadata, CaptureProvider, CaptureRequest,
    DisplayId, DisplayInfo, ErrorCode, ImageId, PixelRect, PixelSize, PinoraError, RgbaBuffer,
};
use xcap::Monitor;

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

    fn capture(&self, request: CaptureRequest) -> Result<CaptureImage, PinoraError> {
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
        let pixels = RgbaBuffer::new(size, bytes).map_err(|msg| {
            PinoraError::new(ErrorCode::Internal, format!("xcap buffer: {msg}"))
        })?;

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
        assert_eq!(
            image.pixels.byte_len(),
            (image.size().area() * 4) as usize
        );
    }
}
