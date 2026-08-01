//! KDE / KWin 快速截图后端。
//!
//! 不走 xdg-desktop-portal / PipeWire（xcap 在 Wayland 上的慢路径）。
//! 当前实现：调用本机 `spectacle`（与 KDE 同源，走 KWin ScreenShot2），
//! 实测全桌面约 0.5s 级，远快于 portal 数秒。
//!
//! 后续可升级为直接 `org.kde.KWin.ScreenShot2` D-Bus（需 .desktop 授权），
//! 去掉进程启动开销。

use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use pinora_core::{
    CaptureImage, CaptureMetadata, CaptureProvider, CaptureRequest, DisplayId, DisplayInfo,
    ErrorCode, ImageId, PinoraError, PixelRect, PixelSize, RgbaBuffer, resolve_capture_rect,
};

/// 基于 KDE Spectacle CLI 的捕获提供者。
#[derive(Debug, Clone)]
pub struct KdeSpectacleCaptureProvider {
    spectacle_bin: PathBuf,
}

impl KdeSpectacleCaptureProvider {
    pub fn new(spectacle_bin: PathBuf) -> Self {
        Self { spectacle_bin }
    }

    /// 探测：KDE 会话 + 可执行 spectacle。
    pub fn probe_available() -> Result<Self, PinoraError> {
        if !is_kde_session() {
            return Err(PinoraError::new(
                ErrorCode::CapabilityUnavailable,
                "not a KDE session",
            ));
        }
        let bin = which_spectacle().ok_or_else(|| {
            PinoraError::new(
                ErrorCode::CapabilityUnavailable,
                "spectacle binary not found in PATH",
            )
        })?;
        // 轻量探测：能列出显示器即可（不真正截图）。
        let displays = list_displays_kscreen().or_else(|_| list_displays_xrandr_like())?;
        if displays.is_empty() {
            return Err(PinoraError::new(
                ErrorCode::CapabilityUnavailable,
                "no displays found via kscreen",
            ));
        }
        Ok(Self::new(bin))
    }

    /// 截取单显示器。优先 `-m`（当前监视器，约 0.4s），比 `-f` 全桌面快且 PNG 更小。
    fn capture_monitor_png(&self) -> Result<PathBuf, PinoraError> {
        let path = std::env::temp_dir().join(format!(
            "pinora-cap-{}-{}.png",
            std::process::id(),
            now_ms()
        ));
        // -m = current monitor；overlay 只盖一块屏时足够，比 -f 快一截
        let status = Command::new(&self.spectacle_bin)
            .args(["-b", "-n", "-m", "-o"])
            .arg(&path)
            .status()
            .map_err(|e| {
                PinoraError::new(
                    ErrorCode::RetryablePlatform,
                    format!("failed to spawn spectacle: {e}"),
                )
            })?;
        if !status.success() || !path.is_file() {
            // 回退全桌面
            let _ = std::fs::remove_file(&path);
            return self.capture_workspace_png();
        }
        Ok(path)
    }

    fn capture_workspace_png(&self) -> Result<PathBuf, PinoraError> {
        let path = std::env::temp_dir().join(format!(
            "pinora-cap-{}-{}.png",
            std::process::id(),
            now_ms()
        ));
        let status = Command::new(&self.spectacle_bin)
            .args([
                "-b", // background, no GUI
                "-n", // no notification
                "-f", // full desktop (all screens)
                "-o",
            ])
            .arg(&path)
            .status()
            .map_err(|e| {
                PinoraError::new(
                    ErrorCode::RetryablePlatform,
                    format!("failed to spawn spectacle: {e}"),
                )
            })?;
        if !status.success() {
            let _ = std::fs::remove_file(&path);
            return Err(PinoraError::new(
                ErrorCode::RetryablePlatform,
                format!("spectacle exited with {status}"),
            ));
        }
        if !path.is_file() {
            return Err(PinoraError::new(
                ErrorCode::RetryablePlatform,
                "spectacle did not write output file",
            ));
        }
        Ok(path)
    }
}

impl CaptureProvider for KdeSpectacleCaptureProvider {
    fn displays(&self) -> Result<Vec<DisplayInfo>, PinoraError> {
        list_displays_kscreen().or_else(|_| list_displays_xrandr_like())
    }

    fn capture(&self, request: CaptureRequest) -> Result<CaptureImage, PinoraError> {
        let displays = self.displays()?;
        let (info, rect) = resolve_capture_rect(&displays, &request)?;

        // 单屏 FullDisplay：用 -m 快路径；区域或需全桌面坐标系时用 -f。
        let want_monitor_only =
            matches!(request, CaptureRequest::FullDisplay { .. }) && rect == info.bounds;

        let png_path = if want_monitor_only {
            self.capture_monitor_png()?
        } else {
            self.capture_workspace_png()?
        };
        let load_result = load_png_rgba(&png_path);
        let _ = std::fs::remove_file(&png_path);
        let (img_size, bytes) = load_result?;

        if want_monitor_only {
            // -m 返回的就是目标屏像素；source_rect 用显示器全局 bounds
            // 若尺寸与 bounds 不一致（缩放/旋转），以实际像素为准并贴在 display origin
            let source = PixelRect::new(
                info.bounds.origin.x,
                info.bounds.origin.y,
                img_size.width,
                img_size.height,
            );
            return CaptureImage::new(
                ImageId::new(),
                RgbaBuffer::new(img_size, bytes)
                    .map_err(|m| PinoraError::new(ErrorCode::Internal, m))?,
                source,
                CaptureMetadata::new(info.id, info.scale, now_ms()),
            )
            .map_err(|m| PinoraError::new(ErrorCode::Internal, m));
        }

        // 全桌面：按全局坐标裁剪
        let workspace_origin = workspace_origin(&displays);
        let local = PixelRect::new(
            rect.origin.x - workspace_origin.0,
            rect.origin.y - workspace_origin.1,
            rect.size.width,
            rect.size.height,
        );

        let full = CaptureImage::new(
            ImageId::new(),
            RgbaBuffer::new(img_size, bytes)
                .map_err(|m| PinoraError::new(ErrorCode::Internal, m))?,
            PixelRect::new(
                workspace_origin.0,
                workspace_origin.1,
                img_size.width,
                img_size.height,
            ),
            CaptureMetadata::new(info.id.clone(), info.scale, now_ms()),
        )
        .map_err(|m| PinoraError::new(ErrorCode::Internal, m))?;

        if local.origin.x == 0 && local.origin.y == 0 && local.size == img_size {
            return Ok(full);
        }

        full.crop_local(local).map(|mut img| {
            img.metadata = CaptureMetadata::new(info.id, info.scale, now_ms());
            img
        })
    }
}

fn is_kde_session() -> bool {
    let desk = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session = std::env::var("DESKTOP_SESSION").unwrap_or_default();
    let upper = format!("{desk};{session}").to_ascii_uppercase();
    upper.contains("KDE") || upper.contains("PLASMA")
}

fn which_spectacle() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("PINORA_SPECTACLE") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("spectacle");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // 常见绝对路径
    for p in ["/usr/bin/spectacle", "/usr/local/bin/spectacle"] {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn workspace_origin(displays: &[DisplayInfo]) -> (i32, i32) {
    displays
        .iter()
        .map(|d| (d.bounds.origin.x, d.bounds.origin.y))
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1)))
        .unwrap_or((0, 0))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn load_png_rgba(path: &std::path::Path) -> Result<(PixelSize, Vec<u8>), PinoraError> {
    let data = std::fs::read(path)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("read png failed: {e}")))?;
    let decoder = png::Decoder::new(Cursor::new(data));
    let mut reader = decoder
        .read_info()
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("png decode header: {e}")))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("png decode frame: {e}")))?;
    let width = info.width;
    let height = info.height;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let rgb = &buf[..info.buffer_size()];
            let mut out = Vec::with_capacity(rgb.len() / 3 * 4);
            for chunk in rgb.chunks_exact(3) {
                out.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            out
        }
        other => {
            return Err(PinoraError::new(
                ErrorCode::Internal,
                format!("unsupported png color type: {other:?}"),
            ));
        }
    };
    Ok((PixelSize::new(width, height), rgba))
}

/// 解析 `kscreen-doctor -o` 文本输出。
fn list_displays_kscreen() -> Result<Vec<DisplayInfo>, PinoraError> {
    let output = Command::new("kscreen-doctor")
        .arg("-o")
        .output()
        .map_err(|e| {
            PinoraError::new(
                ErrorCode::CapabilityUnavailable,
                format!("kscreen-doctor: {e}"),
            )
        })?;
    if !output.status.success() {
        return Err(PinoraError::new(
            ErrorCode::CapabilityUnavailable,
            "kscreen-doctor failed",
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let text = strip_ansi(&text);
    parse_kscreen_doctor(&text)
}

/// 去掉 kscreen-doctor 默认输出的 ANSI 颜色码。
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // ESC [ ... letter
            if chars.peek() == Some(&'[') {
                chars.next();
                for x in chars.by_ref() {
                    if x.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn parse_kscreen_doctor(text: &str) -> Result<Vec<DisplayInfo>, PinoraError> {
    let mut displays = Vec::new();
    let mut cur_name: Option<String> = None;
    let mut cur_id: Option<String> = None;
    let mut cur_geom: Option<(i32, i32, u32, u32)> = None;
    let mut cur_scale: f64 = 1.0;
    let mut cur_enabled = true;

    let flush = |displays: &mut Vec<DisplayInfo>,
                 name: &mut Option<String>,
                 id: &mut Option<String>,
                 geom: &mut Option<(i32, i32, u32, u32)>,
                 scale: &mut f64,
                 enabled: &mut bool| {
        if let (Some(n), Some(g)) = (name.take(), geom.take())
            && *enabled
        {
            let (x, y, w, h) = g;
            if w > 0 && h > 0 {
                let disp_id = id.take().unwrap_or_else(|| format!("kde-{n}"));
                displays.push(DisplayInfo {
                    id: DisplayId::new(disp_id),
                    name: n,
                    bounds: PixelRect::new(x, y, w, h),
                    scale: if scale.is_finite() && *scale > 0.0 {
                        *scale
                    } else {
                        1.0
                    },
                });
            }
        }
        *id = None;
        *scale = 1.0;
        *enabled = true;
    };

    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Output:") {
            flush(
                &mut displays,
                &mut cur_name,
                &mut cur_id,
                &mut cur_geom,
                &mut cur_scale,
                &mut cur_enabled,
            );
            // "1 HDMI-A-1 uuid"
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                cur_id = Some(format!("kde-{}", parts[0]));
                cur_name = Some(parts[1].to_string());
            } else if parts.len() == 1 {
                cur_id = Some(format!("kde-{}", parts[0]));
                cur_name = Some(format!("Output-{}", parts[0]));
            }
        } else if line.starts_with("enabled") {
            cur_enabled = !line.contains("disabled") && !line.contains("false");
            if line == "enabled" {
                cur_enabled = true;
            }
        } else if line.starts_with("disabled") {
            cur_enabled = false;
        } else if let Some(rest) = line.strip_prefix("Geometry:") {
            // "0,0 1440x2560"
            let rest = rest.trim();
            if let Some((xy, wh)) = rest.split_once(char::is_whitespace) {
                let mut xy_it = xy.split(',');
                let x: i32 = xy_it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                let y: i32 = xy_it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                if let Some((w, h)) = wh.split_once('x') {
                    let w: u32 = w.parse().unwrap_or(0);
                    let h: u32 = h.parse().unwrap_or(0);
                    cur_geom = Some((x, y, w, h));
                }
            }
        } else if let Some(rest) = line.strip_prefix("Scale:") {
            cur_scale = rest.trim().parse().unwrap_or(1.0);
        }
    }
    flush(
        &mut displays,
        &mut cur_name,
        &mut cur_id,
        &mut cur_geom,
        &mut cur_scale,
        &mut cur_enabled,
    );

    if displays.is_empty() {
        return Err(PinoraError::new(
            ErrorCode::CapabilityUnavailable,
            "kscreen-doctor returned no enabled outputs",
        ));
    }
    Ok(displays)
}

/// 极简兜底：单虚拟屏（尺寸未知时用常见值；真正裁剪以 PNG 为准）。
fn list_displays_xrandr_like() -> Result<Vec<DisplayInfo>, PinoraError> {
    // 若 kscreen 不可用，返回一个大虚拟桌面占位；capture 时以 PNG 实际尺寸裁剪。
    Ok(vec![DisplayInfo {
        id: DisplayId::new("kde-workspace"),
        name: "Workspace".into(),
        bounds: PixelRect::new(0, 0, 3840, 2160),
        scale: 1.0,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kscreen_sample() {
        let sample = r#"
Output: 1 HDMI-A-1 8fbf4e64-f741-4b39-a8eb-99b6ddff2b82
enabled
connected
priority 2
Geometry: 0,0 1440x2560
Scale: 1
Output: 2 DP-1 c14306ba-42bb-426b-a2e9-f14cc760525c
enabled
connected
priority 1
Geometry: 1440,0 3840x2160
Scale: 1
"#;
        let d = parse_kscreen_doctor(sample).unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].name, "HDMI-A-1");
        assert_eq!(d[0].bounds, PixelRect::new(0, 0, 1440, 2560));
        assert_eq!(d[1].name, "DP-1");
        assert_eq!(d[1].bounds, PixelRect::new(1440, 0, 3840, 2160));
    }

    #[test]
    fn strip_ansi_and_parse() {
        let colored = "\u{1b}[01;32mOutput:\u{1b}[0;0m 1 HDMI-A-1 uuid\n\t\u{1b}[01;32menabled\u{1b}[0;0m\n\tGeometry: 0,0 1440x2560\n\tScale: 1\n";
        let plain = strip_ansi(colored);
        let d = parse_kscreen_doctor(&plain).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].name, "HDMI-A-1");
        assert_eq!(d[0].bounds, PixelRect::new(0, 0, 1440, 2560));
    }

    #[test]
    fn is_kde_detects_env() {
        // 仅检查函数可调用；环境由运行机决定
        let _ = is_kde_session();
    }
}
