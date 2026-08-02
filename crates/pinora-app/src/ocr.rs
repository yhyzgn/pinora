//! 本地 OCR：通过系统 `tesseract` CLI 识别（不链 C++ 库）。
//!
//! 缺引擎或模型时返回可理解错误；不自动联网下载。

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use pinora_core::{
    CaptureImage, ErrorCode, OcrLanguage, OcrLine, OcrResult, OcrWord, PinoraError, PixelRect,
    union_bboxes,
};

use crate::job_supervisor::JobCancellation;

const TESSERACT_TIMEOUT: Duration = Duration::from_secs(30);
const TESSERACT_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_TSV_BYTES: usize = 16 * 1024 * 1024;

/// 探测 tesseract 是否可用。
pub fn tesseract_available() -> bool {
    which("tesseract").is_some()
}

/// 对截图做 OCR，使用默认的自动语言预设。
pub fn recognize_image(image: &CaptureImage) -> Result<OcrResult, PinoraError> {
    let cancellation = JobCancellation::standalone();
    recognize_image_with_language(image, OcrLanguage::Auto, &cancellation)
}

/// 可被任务监督器取消的 OCR 入口，保留默认自动语言以兼容既有调用方。
pub fn recognize_image_with_cancellation(
    image: &CaptureImage,
    cancellation: &JobCancellation,
) -> Result<OcrResult, PinoraError> {
    recognize_image_with_language(image, OcrLanguage::Auto, cancellation)
}

/// 可被任务监督器取消且已冻结语言预设的 OCR 入口。
pub(crate) fn recognize_image_with_language(
    image: &CaptureImage,
    language: OcrLanguage,
    cancellation: &JobCancellation,
) -> Result<OcrResult, PinoraError> {
    if cancellation.is_cancelled() {
        return Err(PinoraError::new(
            ErrorCode::Cancelled,
            "ocr cancelled before start",
        ));
    }
    let bin = which("tesseract").ok_or_else(|| {
        PinoraError::new(
            ErrorCode::Internal,
            "未找到 tesseract：请安装 CLI，例如 `sudo dnf install tesseract tesseract-langpack-eng tesseract-langpack-chi_sim`",
        )
    })?;

    let langs = resolve_languages(language, &list_tesseract_langs())?;
    let lang_arg = langs.join("+");

    let png = encode_png_bytes(image)?;
    let tmp = write_temp_png(&png)?;

    let output = run_tesseract_tsv(&bin, tmp.as_path(), &lang_arg, cancellation)?;

    let lines = parse_tsv(&output)?;
    Ok(OcrResult::from_lines(
        lines,
        langs,
        format!("tesseract-cli:{lang_arg}"),
    ))
}

fn resolve_languages(language: OcrLanguage, listed: &[String]) -> Result<Vec<String>, PinoraError> {
    let has_english = listed.iter().any(|item| item == "eng");
    let has_simplified_chinese = listed.iter().any(|item| item == "chi_sim");

    match language {
        OcrLanguage::Auto => {
            let mut languages = Vec::new();
            if has_simplified_chinese {
                languages.push("chi_sim".into());
            }
            if has_english {
                languages.push("eng".into());
            }
            if languages.is_empty() {
                return Err(PinoraError::new(
                    ErrorCode::CapabilityUnavailable,
                    "OCR 自动语言需要本机安装 eng 或 chi_sim 模型",
                ));
            }
            Ok(languages)
        }
        OcrLanguage::English if has_english => Ok(vec!["eng".into()]),
        OcrLanguage::SimplifiedChinese if has_simplified_chinese => Ok(vec!["chi_sim".into()]),
        OcrLanguage::English => Err(PinoraError::new(
            ErrorCode::CapabilityUnavailable,
            "OCR English 预设需要本机安装 eng 模型",
        )),
        OcrLanguage::SimplifiedChinese => Err(PinoraError::new(
            ErrorCode::CapabilityUnavailable,
            "OCR SimplifiedChinese 预设需要本机安装 chi_sim 模型",
        )),
    }
}

fn list_tesseract_langs() -> Vec<String> {
    let Some(bin) = which("tesseract") else {
        return Vec::new();
    };
    let child = match Command::new(bin)
        .arg("--list-langs")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return probe_tessdata_files(),
    };
    let cancellation = JobCancellation::standalone();
    let Ok(out) = wait_for_child(child, &cancellation, TESSERACT_PROBE_TIMEOUT) else {
        return probe_tessdata_files();
    };
    if !out.status.success() {
        return probe_tessdata_files();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut langs: Vec<String> = text
        .lines()
        .skip(1) // "List of available languages..."
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if langs.is_empty() {
        langs = probe_tessdata_files();
    }
    langs
}

fn probe_tessdata_files() -> Vec<String> {
    let dirs = [
        "/usr/share/tesseract/tessdata",
        "/usr/share/tessdata",
        "/usr/share/tesseract-ocr/5/tessdata",
        "/usr/share/tesseract-ocr/4.00/tessdata",
    ];
    let mut langs = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let name = ent.file_name();
            let name = name.to_string_lossy();
            if let Some(stem) = name.strip_suffix(".traineddata")
                && stem != "osd"
                && stem != "equ"
            {
                langs.push(stem.to_string());
            }
        }
    }
    langs
}

fn run_tesseract_tsv(
    bin: &Path,
    image: &Path,
    langs: &str,
    cancellation: &JobCancellation,
) -> Result<String, PinoraError> {
    // tesseract img stdout -l lang tsv
    let child = Command::new(bin)
        .args([
            image.to_str().unwrap_or(""),
            "stdout",
            "-l",
            langs,
            "--psm",
            "3",
            "tsv",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("spawn tesseract: {e}")))?;

    let output = wait_for_child(child, cancellation, TESSERACT_TIMEOUT)?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(PinoraError::new(
            ErrorCode::Internal,
            format!("tesseract failed: {}", err.trim()),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[derive(Debug)]
struct ChildOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// 等待并回收一个由当前适配器创建的 child。
///
/// stdout/stderr 在独立读取线程中排空，主线程只负责取消、超时和 `wait`；因此
/// 子进程无论哪条错误路径都不会把句柄遗留给调用方。
fn wait_for_child(
    child: Child,
    cancellation: &JobCancellation,
    timeout: Duration,
) -> Result<ChildOutput, PinoraError> {
    wait_for_child_with_limit(child, cancellation, timeout, MAX_TSV_BYTES)
}

fn wait_for_child_with_limit(
    mut child: Child,
    cancellation: &JobCancellation,
    timeout: Duration,
    output_limit: usize,
) -> Result<ChildOutput, PinoraError> {
    if cancellation.is_cancelled() {
        terminate_child(&mut child);
        return Err(PinoraError::new(ErrorCode::Cancelled, "ocr cancelled"));
    }

    let stdout = child.stdout.take().ok_or_else(|| {
        terminate_child(&mut child);
        PinoraError::new(ErrorCode::Internal, "tesseract stdout pipe missing")
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        terminate_child(&mut child);
        PinoraError::new(ErrorCode::Internal, "tesseract stderr pipe missing")
    })?;
    let output_exceeded = Arc::new(AtomicBool::new(false));
    let stdout_thread = spawn_pipe_reader(stdout, output_exceeded.clone(), output_limit);
    let stderr_thread = spawn_pipe_reader(stderr, output_exceeded.clone(), output_limit);
    let started = Instant::now();

    let status = loop {
        if cancellation.is_cancelled() {
            terminate_child(&mut child);
            join_pipe_readers(stdout_thread, stderr_thread);
            return Err(PinoraError::new(ErrorCode::Cancelled, "ocr cancelled"));
        }
        if output_exceeded.load(Ordering::Acquire) {
            terminate_child(&mut child);
            join_pipe_readers(stdout_thread, stderr_thread);
            return Err(PinoraError::new(
                ErrorCode::ResourceLimitExceeded,
                format!("tesseract output exceeded {output_limit} bytes"),
            ));
        }
        if started.elapsed() >= timeout {
            terminate_child(&mut child);
            join_pipe_readers(stdout_thread, stderr_thread);
            return Err(PinoraError::new(
                ErrorCode::TimedOut,
                format!("tesseract timed out ({}s)", timeout.as_secs()),
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(CHILD_POLL_INTERVAL),
            Err(error) => {
                terminate_child(&mut child);
                join_pipe_readers(stdout_thread, stderr_thread);
                return Err(PinoraError::new(
                    ErrorCode::Internal,
                    format!("tesseract wait: {error}"),
                ));
            }
        }
    };

    let stdout = join_pipe_reader(stdout_thread)?;
    let stderr = join_pipe_reader(stderr_thread)?;
    if output_exceeded.load(Ordering::Acquire) {
        return Err(PinoraError::new(
            ErrorCode::ResourceLimitExceeded,
            format!("tesseract output exceeded {output_limit} bytes"),
        ));
    }
    Ok(ChildOutput {
        status,
        stdout,
        stderr,
    })
}

fn spawn_pipe_reader<R>(
    reader: R,
    output_exceeded: Arc<AtomicBool>,
    output_limit: usize,
) -> JoinHandle<std::io::Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut bytes = Vec::new();
        reader
            .take((output_limit + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > output_limit {
            output_exceeded.store(true, Ordering::Release);
            bytes.truncate(output_limit);
        }
        Ok(bytes)
    })
}

fn join_pipe_reader(reader: JoinHandle<std::io::Result<Vec<u8>>>) -> Result<Vec<u8>, PinoraError> {
    reader
        .join()
        .map_err(|_| PinoraError::new(ErrorCode::Internal, "tesseract output reader panicked"))?
        .map_err(|error| {
            PinoraError::new(
                ErrorCode::Internal,
                format!("read tesseract output: {error}"),
            )
        })
}

fn join_pipe_readers(
    stdout: JoinHandle<std::io::Result<Vec<u8>>>,
    stderr: JoinHandle<std::io::Result<Vec<u8>>>,
) {
    let _ = stdout.join();
    let _ = stderr.join();
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// 解析 tesseract TSV（level=5 为 word）。
pub fn parse_tsv(tsv: &str) -> Result<Vec<OcrLine>, PinoraError> {
    let mut lines: Vec<OcrLine> = Vec::new();
    let mut current_key: Option<(i32, i32, i32, i32)> = None; // block,par,line page ignored
    let mut current_words: Vec<OcrWord> = Vec::new();

    for (i, row) in tsv.lines().enumerate() {
        if i == 0 {
            continue; // header
        }
        if row.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = row.split('\t').collect();
        if cols.len() < 12 {
            continue;
        }
        let level: i32 = cols[0].parse().unwrap_or(0);
        if level != 5 {
            continue;
        }
        let block: i32 = cols[2].parse().unwrap_or(0);
        let par: i32 = cols[3].parse().unwrap_or(0);
        let line: i32 = cols[4].parse().unwrap_or(0);
        let left: i32 = cols[6].parse().unwrap_or(0);
        let top: i32 = cols[7].parse().unwrap_or(0);
        let width: u32 = cols[8].parse().unwrap_or(0);
        let height: u32 = cols[9].parse().unwrap_or(0);
        let conf: f32 = cols[10].parse().unwrap_or(-1.0);
        let text = cols[11..].join("\t").trim().to_string();
        if text.is_empty() {
            continue;
        }
        let key = (block, par, line, 0);
        if current_key != Some(key) {
            if !current_words.is_empty() {
                let bbox = union_bboxes(current_words.iter().map(|w| w.bbox));
                lines.push(OcrLine {
                    words: std::mem::take(&mut current_words),
                    bbox,
                });
            }
            current_key = Some(key);
        }
        current_words.push(OcrWord {
            text,
            confidence: conf,
            bbox: PixelRect::new(left, top, width, height),
        });
    }
    if !current_words.is_empty() {
        let bbox = union_bboxes(current_words.iter().map(|w| w.bbox));
        lines.push(OcrLine {
            words: current_words,
            bbox,
        });
    }
    Ok(lines)
}

fn encode_png_bytes(image: &CaptureImage) -> Result<Vec<u8>, PinoraError> {
    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let width = image.pixels.size.width;
        let height = image.pixels.size.height;
        let mut encoder = png::Encoder::new(cursor, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("ocr png header: {e}")))?;
        writer
            .write_image_data(&image.pixels.bytes)
            .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("ocr png data: {e}")))?;
    }
    Ok(buf)
}

struct TempPng {
    path: PathBuf,
}

impl TempPng {
    fn as_path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempPng {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn write_temp_png(png: &[u8]) -> Result<TempPng, PinoraError> {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "pinora-ocr-{}-{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let mut f = std::fs::File::create(&path)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("ocr temp: {e}")))?;
    f.write_all(png)
        .map_err(|e| PinoraError::new(ErrorCode::Internal, format!("ocr temp write: {e}")))?;
    Ok(TempPng { path })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn running_shell() -> Child {
        Command::new("sh")
            .args(["-c", "while true; do :; done"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn local shell")
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_reaps_owned_child_without_external_kill() {
        use pinora_core::{
            AssetRef, CorrelationId, ImageId, JobId, JobKind, JobOwner, JobSpec, SessionId,
        };

        let asset = AssetRef::initial(ImageId::from_raw(901));
        let spec = JobSpec::new(
            JobId::from_raw(901),
            CorrelationId::from_raw(901),
            asset,
            JobOwner::Session(SessionId::from_raw(901)),
            JobKind::Ocr,
            u64::MAX,
        );
        let mut supervisor = crate::job_supervisor::JobSupervisor::new();
        let ticket = supervisor.submit(spec).expect("submit job");
        let cancellation = ticket.cancellation();
        supervisor.cancel(ticket.id).expect("cancel job");

        let error = wait_for_child(running_shell(), &cancellation, Duration::from_secs(2))
            .expect_err("cancelled child must not complete");
        assert_eq!(error.code, ErrorCode::Cancelled);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_reaps_owned_child() {
        let cancellation = JobCancellation::standalone();
        let error = wait_for_child(running_shell(), &cancellation, Duration::from_millis(30))
            .expect_err("timed out child must not complete");
        assert_eq!(error.code, ErrorCode::TimedOut);
    }

    #[test]
    fn output_reader_marks_and_truncates_over_limit() {
        let output_exceeded = Arc::new(AtomicBool::new(false));
        let input = std::io::Cursor::new(vec![b'x'; MAX_TSV_BYTES + 1]);
        let reader = spawn_pipe_reader(input, output_exceeded.clone(), MAX_TSV_BYTES);
        let bytes = join_pipe_reader(reader).expect("reader");

        assert_eq!(bytes.len(), MAX_TSV_BYTES);
        assert!(output_exceeded.load(Ordering::Acquire));
    }

    #[cfg(unix)]
    #[test]
    fn over_limit_output_is_reaped_and_has_stable_error() {
        let child = Command::new("sh")
            .args(["-c", "printf 123456789"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn local shell");
        let cancellation = JobCancellation::standalone();

        let error = wait_for_child_with_limit(child, &cancellation, Duration::from_secs(2), 8)
            .expect_err("over-limit output must be rejected");
        assert_eq!(error.code, ErrorCode::ResourceLimitExceeded);
    }

    #[test]
    fn temporary_png_is_removed_when_scope_ends() {
        let temp = write_temp_png(b"test png bytes").expect("temp file");
        let path = temp.as_path().to_path_buf();
        assert!(path.is_file());
        drop(temp);
        assert!(!path.exists());
    }

    #[test]
    fn parse_sample_tsv() {
        let tsv = "\
level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext
1\t1\t0\t0\t0\t0\t0\t0\t100\t50\t-1\t
5\t1\t1\t1\t1\t1\t10\t10\t20\t12\t96.5\tHello
5\t1\t1\t1\t1\t2\t35\t10\t18\t12\t94.0\tWorld
5\t1\t1\t1\t2\t1\t10\t30\t25\t12\t91.0\tOCR
";
        let lines = parse_tsv(tsv).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text(), "Hello World");
        assert_eq!(lines[1].text(), "OCR");
        let r = OcrResult::from_lines(lines, vec!["eng".into()], "test");
        assert_eq!(r.full_text, "Hello World\nOCR");
    }

    #[test]
    fn auto_language_uses_only_supported_local_models_in_stable_order() {
        let languages = resolve_languages(
            OcrLanguage::Auto,
            &["eng".into(), "deu".into(), "chi_sim".into()],
        )
        .expect("supported local models");

        assert_eq!(languages, vec!["chi_sim", "eng"]);
    }

    #[test]
    fn explicit_language_requires_its_exact_local_model() {
        let english_missing = resolve_languages(OcrLanguage::English, &["chi_sim".into()])
            .expect_err("English must not fall back to Chinese");
        assert_eq!(english_missing.code, ErrorCode::CapabilityUnavailable);

        let chinese_missing = resolve_languages(OcrLanguage::SimplifiedChinese, &["eng".into()])
            .expect_err("SimplifiedChinese must not fall back to English");
        assert_eq!(chinese_missing.code, ErrorCode::CapabilityUnavailable);

        let automatic_missing =
            resolve_languages(OcrLanguage::Auto, &["deu".into()]).expect_err("no supported model");
        assert_eq!(automatic_missing.code, ErrorCode::CapabilityUnavailable);
    }

    #[test]
    fn tesseract_probe_does_not_panic() {
        let _ = tesseract_available();
        let _ = list_tesseract_langs();
    }

    #[test]
    fn live_tesseract_selects_chi_sim_and_eng_when_installed() {
        if !tesseract_available() {
            return;
        }
        let listed = list_tesseract_langs();
        let langs = resolve_languages(OcrLanguage::Auto, &listed).unwrap_or_default();
        assert!(
            langs.iter().any(|l| l == "eng"),
            "expected eng in {langs:?}"
        );
        // 本机已装 chi_sim 时应优先中英
        if listed.iter().any(|l| l == "chi_sim") {
            assert!(
                langs.iter().any(|l| l == "chi_sim"),
                "expected chi_sim in {langs:?}"
            );
        }

        use pinora_core::{
            CaptureImage, CaptureMetadata, DisplayId, ImageId, PixelRect, PixelSize, RgbaBuffer,
        };
        let image = CaptureImage::new(
            ImageId::new(),
            RgbaBuffer::solid(PixelSize::new(64, 32), [255, 255, 255, 255]),
            PixelRect::new(0, 0, 64, 32),
            CaptureMetadata::new(DisplayId::new("d0"), 1.0, 0),
        )
        .unwrap();
        // 白图可能无字，但引擎调用应成功
        let result = recognize_image(&image).expect("tesseract should run");
        assert_eq!(result.languages, langs);
    }
}
