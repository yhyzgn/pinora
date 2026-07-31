//! 本地 PNG 导出 + 内存剪贴板 + 系统剪贴板（Linux：wl-copy / xclip）。

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use pinora_core::{CaptureImage, ErrorCode, ImageId, ImageSink, PinoraError};

/// 将 RGBA 写为 PNG 文件，内存保留最近一次复制，并尽力写入系统剪贴板。
#[derive(Debug, Default)]
pub struct LocalImageSink {
    clipboard: Mutex<Option<CaptureImage>>,
}

impl LocalImageSink {
    pub fn new() -> Self {
        Self {
            clipboard: Mutex::new(None),
        }
    }

    pub fn clipboard_image_id(&self) -> Option<ImageId> {
        self.clipboard
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|img| img.id))
    }

    pub fn clipboard_byte_len(&self) -> Option<usize> {
        self.clipboard
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|img| img.pixels.byte_len()))
    }
}

impl ImageSink for LocalImageSink {
    fn save_png(&self, image: &CaptureImage, path: &Path) -> Result<(), PinoraError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                PinoraError::new(ErrorCode::Internal, format!("create export dir: {e}"))
            })?;
        }
        let file = File::create(path)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("create png: {e}")))?;
        let w = BufWriter::new(file);
        encode_png_to_writer(image, w)
    }

    fn copy_image(&self, image: &CaptureImage) -> Result<(), PinoraError> {
        // 1) 内存副本（测试与降级）
        {
            let mut guard = self
                .clipboard
                .lock()
                .map_err(|_| PinoraError::new(ErrorCode::Internal, "clipboard lock poisoned"))?;
            *guard = Some(image.clone());
        }

        // 2) 系统剪贴板：尽力而为，失败不推翻内存成功
        match encode_png_bytes(image) {
            Ok(png) => match copy_png_to_system_clipboard(&png) {
                Ok(backend) => {
                    println!("pinora: system clipboard ← image/png via {backend}");
                }
                Err(e) => {
                    eprintln!("pinora: system clipboard skipped: {e}");
                }
            },
            Err(e) => {
                eprintln!("pinora: png encode for clipboard failed: {e}");
            }
        }
        Ok(())
    }
}

fn encode_png_to_writer<W: Write>(image: &CaptureImage, w: W) -> Result<(), PinoraError> {
    let width = image.pixels.size.width;
    let height = image.pixels.size.height;
    let mut encoder = png::Encoder::new(w, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| {
        PinoraError::new(ErrorCode::Internal, format!("png header: {e}"))
    })?;
    writer
        .write_image_data(&image.pixels.bytes)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("png data: {e}")))?;
    Ok(())
}

fn encode_png_bytes(image: &CaptureImage) -> Result<Vec<u8>, PinoraError> {
    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        encode_png_to_writer(image, cursor)?;
    }
    Ok(buf)
}

/// 探测并写入系统剪贴板。返回使用的后端名。
fn copy_png_to_system_clipboard(png: &[u8]) -> Result<&'static str, String> {
    // 测试或显式关闭时跳过，避免 wl-copy 在无会话时挂起
    if std::env::var_os("PINORA_NO_SYSTEM_CLIPBOARD").is_some() {
        return Err("disabled by PINORA_NO_SYSTEM_CLIPBOARD".into());
    }
    if let Some(bin) = which("wl-copy") {
        // 默认 fork 后台；不要加 --foreground（会一直等到粘贴，易挂起）
        pipe_to_cmd(&bin, &["--type", "image/png"], png)?;
        return Ok("wl-copy");
    }
    if let Some(bin) = which("xclip") {
        pipe_to_cmd(
            &bin,
            &["-selection", "clipboard", "-t", "image/png", "-i"],
            png,
        )?;
        return Ok("xclip");
    }
    if let Some(bin) = which("xsel") {
        // xsel 对 image/png 支持差，仍尝试
        pipe_to_cmd(&bin, &["--clipboard", "--input"], png)?;
        return Ok("xsel");
    }
    Err("no wl-copy/xclip/xsel in PATH".into())
}

fn pipe_to_cmd(bin: &Path, args: &[&str], data: &[u8]) -> Result<(), String> {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", bin.display()))?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "stdin missing".to_string())?;
        stdin
            .write_all(data)
            .map_err(|e| format!("write stdin: {e}"))?;
        // 显式关闭 stdin，让 wl-copy 结束读取
        drop(stdin);
    }

    // 带超时等待，防止 wl-copy 在异常会话下永久阻塞
    let (tx, rx) = mpsc::channel();
    let child_id = child.id();
    thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(out)) => {
            if !out.status.success() {
                return Err(format!(
                    "{} exit {} stderr={}",
                    bin.display(),
                    out.status,
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            Ok(())
        }
        Ok(Err(e)) => Err(format!("wait: {e}")),
        Err(_) => {
            let _ = Command::new("kill")
                .args(["-9", &child_id.to_string()])
                .status();
            Err(format!("{} timed out after 3s", bin.display()))
        }
    }
}

fn which(name: &str) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("PATH") {
        for dir in std::env::split_paths(&p) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// 当前系统剪贴板后端名称（探测用）。
pub fn detect_system_clipboard_backend() -> Option<&'static str> {
    if which("wl-copy").is_some() {
        Some("wl-copy")
    } else if which("xclip").is_some() {
        Some("xclip")
    } else if which("xsel").is_some() {
        Some("xsel")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{
        CaptureMetadata, DisplayId, ImageId, PixelRect, PixelSize, RgbaBuffer,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_image() -> CaptureImage {
        let pixels = RgbaBuffer::solid(PixelSize::new(8, 4), [10, 20, 30, 255]);
        CaptureImage::new(
            ImageId::new(),
            pixels,
            PixelRect::new(0, 0, 8, 4),
            CaptureMetadata::new(DisplayId::new("d0"), 1.0, 0),
        )
        .unwrap()
    }

    #[test]
    fn save_png_writes_signature() {
        let sink = LocalImageSink::new();
        let image = sample_image();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("pinora-export-{nanos}.png"));
        sink.save_png(&image, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn copy_image_tracks_id() {
        // 系统剪贴板有 3s 超时；内存副本始终写入
        let sink = LocalImageSink::new();
        let image = sample_image();
        let id = image.id;
        sink.copy_image(&image).unwrap();
        assert_eq!(sink.clipboard_image_id(), Some(id));
        assert_eq!(sink.clipboard_byte_len(), Some(8 * 4 * 4));
    }

    #[test]
    fn encode_png_bytes_has_signature() {
        let png = encode_png_bytes(&sample_image()).unwrap();
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']));
    }

    #[test]
    #[ignore = "requires display session and wl-copy/xclip"]
    fn system_clipboard_roundtrip_if_available() {
        let Some(backend) = detect_system_clipboard_backend() else {
            return;
        };
        let sink = LocalImageSink::new();
        sink.copy_image(&sample_image()).unwrap();
        assert!(matches!(backend, "wl-copy" | "xclip" | "xsel"));
    }
}
