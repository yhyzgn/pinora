//! 本地 PNG 导出 + 内存剪贴板 + 系统剪贴板（Linux：wl-copy / xclip）。

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Mutex, atomic::AtomicU64};
use std::time::{Duration, Instant};

use pinora_core::{CaptureImage, ErrorCode, ImageId, ImageSink, PinoraError};

use crate::job_supervisor::JobCancellation;

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
        save_png_file(image, path)
    }

    fn copy_image(&self, image: &CaptureImage) -> Result<(), PinoraError> {
        self.copy_image_with_writer(image, copy_png_to_system_clipboard)
    }
}

impl LocalImageSink {
    fn copy_image_with_writer<F>(
        &self,
        image: &CaptureImage,
        write_system_clipboard: F,
    ) -> Result<(), PinoraError>
    where
        F: FnOnce(&[u8]) -> Result<&'static str, String>,
    {
        // 1) 内存副本（测试与降级）
        {
            let mut guard = self
                .clipboard
                .lock()
                .map_err(|_| PinoraError::new(ErrorCode::Internal, "clipboard lock poisoned"))?;
            *guard = Some(image.clone());
        }

        // 2) 系统剪贴板：内存成功不等于系统成功；失败保留缓存供调用方重试。
        let png = encode_png_bytes(image).map_err(|_| {
            eprintln!("pinora: system clipboard image encoding failed; memory copy retained");
            PinoraError::new(
                ErrorCode::ClipboardFailed,
                "system clipboard image encoding failed",
            )
        })?;
        match write_system_clipboard(&png) {
            Ok(backend) => {
                println!("pinora: system clipboard ← image/png via {backend}");
                Ok(())
            }
            Err(_) => {
                eprintln!("pinora: system clipboard image write failed; memory copy retained");
                Err(PinoraError::new(
                    ErrorCode::ClipboardFailed,
                    "system clipboard image write failed",
                ))
            }
        }
    }
}

fn encode_png_to_writer<W: Write>(image: &CaptureImage, w: W) -> Result<(), PinoraError> {
    let width = image.pixels.size.width;
    let height = image.pixels.size.height;
    let mut encoder = png::Encoder::new(w, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("png header: {e}")))?;
    writer
        .write_image_data(&image.pixels.bytes)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("png data: {e}")))?;
    Ok(())
}

pub(crate) fn encode_png_bytes(image: &CaptureImage) -> Result<Vec<u8>, PinoraError> {
    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        encode_png_to_writer(image, cursor)?;
    }
    Ok(buf)
}

pub(crate) fn save_png_file(image: &CaptureImage, path: &Path) -> Result<(), PinoraError> {
    let mut temporary = AtomicPngTemp::create(path)?;
    let png = encode_png_bytes(image)?;
    let mut file = temporary.take_file()?;
    file.write_all(&png)
        .map_err(|error| PinoraError::new(ErrorCode::Internal, format!("write png: {error}")))?;
    file.sync_all()
        .map_err(|error| PinoraError::new(ErrorCode::Internal, format!("sync png: {error}")))?;
    drop(file);
    temporary.commit(path)
}

static NEXT_EXPORT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(1);

/// 同目录临时 PNG。只有 `commit` 成功后才向目标路径发布；其他路径由 Drop 清理。
#[derive(Debug)]
struct AtomicPngTemp {
    path: PathBuf,
    file: Option<File>,
    committed: bool,
}

impl AtomicPngTemp {
    fn create(target: &Path) -> Result<Self, PinoraError> {
        let directory = target
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(directory).map_err(|error| {
            PinoraError::new(ErrorCode::Internal, format!("create export dir: {error}"))
        })?;

        for _ in 0..16 {
            let id = NEXT_EXPORT_TEMP_FILE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = directory.join(format!(".pinora-export-{}-{id}.tmp", std::process::id()));
            match OpenOptions::new().create_new(true).write(true).open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        committed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(PinoraError::new(
                        ErrorCode::Internal,
                        format!("create export temp: {error}"),
                    ));
                }
            }
        }
        Err(PinoraError::new(
            ErrorCode::Internal,
            "create export temp: collision limit reached",
        ))
    }

    fn take_file(&mut self) -> Result<File, PinoraError> {
        self.file
            .take()
            .ok_or_else(|| PinoraError::new(ErrorCode::Internal, "export temp file already moved"))
    }

    fn commit(mut self, target: &Path) -> Result<(), PinoraError> {
        if self.file.is_some() {
            return Err(PinoraError::new(
                ErrorCode::Internal,
                "export temp file is still open",
            ));
        }
        std::fs::rename(&self.path, target).map_err(|error| {
            PinoraError::new(ErrorCode::Internal, format!("publish png: {error}"))
        })?;
        self.committed = true;
        File::open(target).map_err(|error| {
            PinoraError::new(
                ErrorCode::Internal,
                format!("verify published png: {error}"),
            )
        })?;
        Ok(())
    }
}

impl Drop for AtomicPngTemp {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// 探测并写入系统剪贴板。返回使用的后端名。
fn copy_png_to_system_clipboard(png: &[u8]) -> Result<&'static str, String> {
    copy_png_to_system_clipboard_with_optional_cancellation(png, None)
}

pub(crate) fn copy_png_to_system_clipboard_with_cancellation(
    png: &[u8],
    cancellation: &JobCancellation,
) -> Result<&'static str, String> {
    copy_png_to_system_clipboard_with_optional_cancellation(png, Some(cancellation))
}

fn copy_png_to_system_clipboard_with_optional_cancellation(
    png: &[u8],
    cancellation: Option<&JobCancellation>,
) -> Result<&'static str, String> {
    // 测试或显式关闭时跳过，避免 wl-copy 在无会话时挂起
    if std::env::var_os("PINORA_NO_SYSTEM_CLIPBOARD").is_some() {
        return Err("disabled by PINORA_NO_SYSTEM_CLIPBOARD".into());
    }
    if let Some(bin) = which("wl-copy") {
        // 默认 fork 后台；不要加 --foreground（会一直等到粘贴，易挂起）
        pipe_to_cmd_with_optional_cancellation(&bin, &["--type", "image/png"], png, cancellation)?;
        return Ok("wl-copy");
    }
    if let Some(bin) = which("xclip") {
        pipe_to_cmd_with_optional_cancellation(
            &bin,
            &["-selection", "clipboard", "-t", "image/png", "-i"],
            png,
            cancellation,
        )?;
        return Ok("xclip");
    }
    if let Some(bin) = which("xsel") {
        // xsel 对 image/png 支持差，仍尝试
        pipe_to_cmd_with_optional_cancellation(
            &bin,
            &["--clipboard", "--input"],
            png,
            cancellation,
        )?;
        return Ok("xsel");
    }
    Err("no wl-copy/xclip/xsel in PATH".into())
}

fn pipe_to_cmd_with_optional_cancellation(
    bin: &Path,
    args: &[&str],
    data: &[u8],
    cancellation: Option<&JobCancellation>,
) -> Result<(), String> {
    pipe_to_cmd_with_timeout_and_cancellation(bin, args, data, Duration::from_secs(3), cancellation)
}

const MAX_CLIPBOARD_STDERR: usize = 8 * 1024;
static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(1);

/// 由当前适配器拥有的临时文件。Drop 时清理，避免把管道写入或错误输出留给
/// 独立线程；子进程退出后，普通文件仍可安全读取有限诊断并删除。
#[derive(Debug)]
struct OwnedTempFile {
    path: PathBuf,
    file: Option<File>,
}

impl OwnedTempFile {
    fn create(kind: &str) -> Result<Self, String> {
        for _ in 0..16 {
            let id = NEXT_TEMP_FILE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pinora-clipboard-{}-{}-{id}.tmp",
                std::process::id(),
                kind
            ));
            match OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(&path)
            {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!("create clipboard temp file: {error}"));
                }
            }
        }
        Err("create clipboard temp file: collision limit reached".into())
    }

    fn file_mut(&mut self) -> Result<&mut File, String> {
        self.file
            .as_mut()
            .ok_or_else(|| "clipboard temp file already moved".to_string())
    }

    fn take_file(&mut self) -> Result<File, String> {
        self.file
            .take()
            .ok_or_else(|| "clipboard temp file already moved".to_string())
    }

    fn read_bounded(&self, limit: usize) -> Result<Vec<u8>, String> {
        let mut file =
            File::open(&self.path).map_err(|error| format!("read clipboard stderr: {error}"))?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take((limit as u64).saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| format!("read clipboard stderr: {error}"))?;
        if bytes.len() > limit {
            bytes.truncate(limit);
            bytes.extend_from_slice(b"...<truncated>");
        }
        Ok(bytes)
    }
}

impl Drop for OwnedTempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn pipe_to_cmd_with_timeout_and_cancellation(
    bin: &Path,
    args: &[&str],
    data: &[u8],
    timeout: Duration,
    cancellation: Option<&JobCancellation>,
) -> Result<(), String> {
    let mut stdin_file = OwnedTempFile::create("stdin")?;
    {
        let file = stdin_file.file_mut()?;
        file.write_all(data)
            .map_err(|error| format!("write clipboard stdin: {error}"))?;
        file.flush()
            .map_err(|error| format!("flush clipboard stdin: {error}"))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("rewind clipboard stdin: {error}"))?;
    }
    let mut stderr_file = OwnedTempFile::create("stderr")?;
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::from(stdin_file.take_file()?))
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_file.take_file()?))
        .spawn()
        .map_err(|error| format!("spawn {}: {error}", bin.display()))?;

    let status = wait_for_owned_child(&mut child, timeout, bin, cancellation)?;
    let stderr = stderr_file.read_bounded(MAX_CLIPBOARD_STDERR)?;
    let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
    if status.success() {
        return Ok(());
    }
    if stderr.is_empty() {
        Err(format!("{} exit {status}", bin.display()))
    } else {
        Err(format!("{} exit {status} stderr={stderr}", bin.display()))
    }
}

fn wait_for_owned_child(
    child: &mut Child,
    timeout: Duration,
    bin: &Path,
    cancellation: Option<&JobCancellation>,
) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if cancellation.is_some_and(JobCancellation::is_cancelled) {
            let details = terminate_owned_child(child);
            let suffix = if details.is_empty() {
                "child reaped".to_string()
            } else {
                details.join(", ")
            };
            return Err(format!("{} cancelled ({suffix})", bin.display()));
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() >= deadline => {
                let details = terminate_owned_child(child);
                let suffix = if details.is_empty() {
                    "child reaped".to_string()
                } else {
                    details.join(", ")
                };
                return Err(format!(
                    "{} timed out after {}ms ({suffix})",
                    bin.display(),
                    timeout.as_millis()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                let _ = terminate_owned_child(child);
                return Err(format!("wait {}: {error}", bin.display()));
            }
        }
    }
}

fn terminate_owned_child(child: &mut Child) -> Vec<String> {
    child
        .kill()
        .err()
        .map(|error| format!("kill={error}"))
        .into_iter()
        .chain(child.wait().err().map(|error| format!("wait={error}")))
        .collect()
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

/// 将纯文本写入系统剪贴板（OCR 全文等）。
pub fn copy_text_to_system_clipboard(text: &str) -> Result<&'static str, String> {
    copy_text_to_system_clipboard_with_optional_cancellation(text, None)
}

pub(crate) fn copy_text_to_system_clipboard_with_cancellation(
    text: &str,
    cancellation: &JobCancellation,
) -> Result<&'static str, String> {
    copy_text_to_system_clipboard_with_optional_cancellation(text, Some(cancellation))
}

fn copy_text_to_system_clipboard_with_optional_cancellation(
    text: &str,
    cancellation: Option<&JobCancellation>,
) -> Result<&'static str, String> {
    if std::env::var_os("PINORA_NO_SYSTEM_CLIPBOARD").is_some() {
        return Err("disabled by PINORA_NO_SYSTEM_CLIPBOARD".into());
    }
    let bytes = text.as_bytes();
    if let Some(bin) = which("wl-copy") {
        pipe_to_cmd_with_optional_cancellation(
            &bin,
            &["--type", "text/plain"],
            bytes,
            cancellation,
        )?;
        return Ok("wl-copy");
    }
    if let Some(bin) = which("xclip") {
        pipe_to_cmd_with_optional_cancellation(
            &bin,
            &["-selection", "clipboard", "-t", "text/plain", "-i"],
            bytes,
            cancellation,
        )?;
        return Ok("xclip");
    }
    if let Some(bin) = which("xsel") {
        pipe_to_cmd_with_optional_cancellation(
            &bin,
            &["--clipboard", "--input"],
            bytes,
            cancellation,
        )?;
        return Ok("xsel");
    }
    Err("no wl-copy/xclip/xsel in PATH".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{CaptureMetadata, DisplayId, ImageId, PixelRect, PixelSize, RgbaBuffer};
    #[cfg(unix)]
    use std::time::Duration;
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

    fn temporary_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("pinora-{label}-{nanos}.png"))
    }

    #[test]
    fn save_png_writes_signature() {
        let sink = LocalImageSink::new();
        let image = sample_image();
        let path = temporary_path("export");
        sink.save_png(&image, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn save_png_atomically_replaces_existing_file() {
        let image = sample_image();
        let path = temporary_path("replace");
        std::fs::write(&path, b"old export").expect("write old export");

        save_png_file(&image, &path).expect("atomic save");

        let bytes = std::fs::read(&path).expect("read published png");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']));
        assert_ne!(bytes, b"old export");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn uncommitted_atomic_png_temp_is_removed() {
        let target = temporary_path("uncommitted");
        let temp = AtomicPngTemp::create(&target).expect("create temp");
        let temp_path = temp.path.clone();
        assert!(temp_path.exists());

        drop(temp);

        assert!(!temp_path.exists());
    }

    #[test]
    fn copy_image_tracks_id() {
        // 系统剪贴板有 3s 超时；内存副本始终写入
        let sink = LocalImageSink::new();
        let image = sample_image();
        let id = image.id;
        if let Err(error) = sink.copy_image(&image) {
            assert_eq!(error.code, ErrorCode::ClipboardFailed);
        }
        assert_eq!(sink.clipboard_image_id(), Some(id));
        assert_eq!(sink.clipboard_byte_len(), Some(8 * 4 * 4));
    }

    #[test]
    fn failed_system_copy_retains_memory_image_for_retry() {
        let sink = LocalImageSink::new();
        let image = sample_image();
        let error = sink
            .copy_image_with_writer(&image, |_| Err("injected clipboard failure".into()))
            .expect_err("system copy should fail");

        assert_eq!(error.code, ErrorCode::ClipboardFailed);
        assert_eq!(sink.clipboard_image_id(), Some(image.id));
        assert_eq!(sink.clipboard_byte_len(), Some(image.pixels.byte_len()));
    }

    #[test]
    fn encode_png_bytes_has_signature() {
        let png = encode_png_bytes(&sample_image()).unwrap();
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']));
    }

    #[test]
    #[cfg(unix)]
    fn owned_clipboard_command_reaps_normal_exit() {
        let shell = which("sh").expect("test requires sh");
        pipe_to_cmd_with_timeout_and_cancellation(
            &shell,
            &["-c", "cat >/dev/null"],
            b"clipboard payload",
            Duration::from_secs(1),
            None,
        )
        .expect("fake clipboard command should succeed");
    }

    #[test]
    #[cfg(unix)]
    fn owned_clipboard_command_reaps_timeout() {
        let shell = which("sh").expect("test requires sh");
        let error = pipe_to_cmd_with_timeout_and_cancellation(
            &shell,
            &["-c", "exec sleep 10"],
            b"clipboard payload",
            Duration::from_millis(30),
            None,
        )
        .expect_err("fake clipboard command should time out");
        assert!(error.contains("timed out"), "unexpected error: {error}");
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
