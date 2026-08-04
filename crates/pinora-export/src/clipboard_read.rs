//! 受监督的系统剪贴板图像读取。
//!
//! 读取仅接受 `image/png`，且将外部命令、字节流和像素解码全部限制在 worker 内。
//! GUI 线程只轮询已验证的 `CaptureImage`，不会等待桌面剪贴板服务。

use std::io::Cursor;
use std::sync::{
    Arc,
    mpsc::{self, Receiver, Sender},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "linux")]
use std::fs::{File, OpenOptions};
#[cfg(target_os = "linux")]
use std::io::Read;
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_os = "linux")]
use std::time::Instant;

use pinora_core::{
    CaptureImage, CaptureMetadata, DisplayId, ErrorCode, ImageId, JobId, JobKind, JobOwner,
    JobResultRef, JobSpec, JobTerminalState, PinoraError, PixelRect, PixelSize, RgbaBuffer,
};
use pinora_jobs::{
    AcceptedJobResult, JobCancellation, JobResultDisposition, JobState, JobSupervisor, JobTicket,
    WorkerWaitOutcome, reap_finished_workers, wait_for_workers,
};

#[cfg(target_os = "linux")]
const CLIPBOARD_READ_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_PNG_BYTES: usize = 64 * 1024 * 1024;
const MAX_PIXELS: u64 = 50_000_000;
const MAX_RGBA_BYTES: usize = 200 * 1024 * 1024;
#[cfg(target_os = "linux")]
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

/// 可替换的系统图像剪贴板读取端口。
pub trait ClipboardImageReader: Send + Sync + 'static {
    fn read_image(
        &self,
        image_id: ImageId,
        cancellation: &JobCancellation,
    ) -> Result<CaptureImage, PinoraError>;
}

/// 本机实现：Linux 优先 `wl-paste`，回退到 X11 `xclip`；其他平台明确受限。
#[derive(Debug, Default)]
pub struct LocalClipboardImageReader;

impl ClipboardImageReader for LocalClipboardImageReader {
    fn read_image(
        &self,
        image_id: ImageId,
        cancellation: &JobCancellation,
    ) -> Result<CaptureImage, PinoraError> {
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let png = read_clipboard_png(cancellation)?;
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        decode_clipboard_png(image_id, &png)
    }
}

#[derive(Debug)]
struct WorkerResult {
    reference: JobResultRef,
    result: Result<CaptureImage, PinoraError>,
}

/// 剪贴板读取完成事件。像素只在成功完成时回到 GUI 线程。
#[derive(Debug)]
pub enum ClipboardImageReadCompletion {
    Completed {
        job: AcceptedJobResult,
        image: CaptureImage,
    },
    Failed {
        job_id: JobId,
        owner: JobOwner,
        error: PinoraError,
    },
    Discarded {
        job_id: JobId,
        owner: JobOwner,
        terminal: JobTerminalState,
    },
}

/// 管理剪贴板导入线程、截止时间、取消和结果门禁。
pub struct ClipboardImageReadJobService<R: ClipboardImageReader = LocalClipboardImageReader> {
    supervisor: JobSupervisor,
    reader: Arc<R>,
    sender: Sender<WorkerResult>,
    receiver: Receiver<WorkerResult>,
    workers: Vec<JoinHandle<()>>,
}

impl ClipboardImageReadJobService<LocalClipboardImageReader> {
    pub fn new() -> Self {
        Self::with_reader(LocalClipboardImageReader)
    }
}

impl Default for ClipboardImageReadJobService<LocalClipboardImageReader> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: ClipboardImageReader> ClipboardImageReadJobService<R> {
    pub fn with_reader(reader: R) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            supervisor: JobSupervisor::new(),
            reader: Arc::new(reader),
            sender,
            receiver,
            workers: Vec::new(),
        }
    }

    pub fn start(&mut self, spec: JobSpec) -> Result<JobTicket, PinoraError> {
        if spec.kind != JobKind::Clipboard || spec.owner != JobOwner::Clipboard(spec.asset.image_id)
        {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                "clipboard image read requires a matching clipboard job owner",
            ));
        }
        let ticket = self.supervisor.submit(spec)?;
        let cancellation = ticket.cancellation();
        let sender = self.sender.clone();
        let reader = self.reader.clone();
        let reference = JobResultRef::new(ticket.id, spec.asset);
        let worker = thread::Builder::new()
            .name(format!("pinora-clipboard-read-{}", ticket.id.raw()))
            .spawn(move || {
                let result = reader.read_image(spec.asset.image_id, &cancellation);
                let _ = sender.send(WorkerResult { reference, result });
            })
            .map_err(|error| {
                let _ = self.supervisor.fail(ticket.id);
                PinoraError::new(
                    ErrorCode::Internal,
                    format!("spawn clipboard reader: {error}"),
                )
            })?;
        self.workers.push(worker);
        Ok(ticket)
    }

    pub fn cancel_all(&mut self) -> usize {
        self.supervisor.cancel_all()
    }

    pub fn cancel_all_and_wait(&mut self, timeout: Duration) -> WorkerWaitOutcome {
        let cancelled = self.cancel_all();
        let mut outcome = wait_for_workers(&mut self.workers, timeout);
        outcome.cancelled = cancelled;
        outcome
    }

    pub fn poll(&mut self, now_ms: u64) -> Vec<ClipboardImageReadCompletion> {
        self.supervisor.expire_at(now_ms);
        let mut completions = Vec::new();
        while let Ok(worker) = self.receiver.try_recv() {
            let Some(spec) = self.supervisor.spec(worker.reference.job_id) else {
                continue;
            };
            match worker.result {
                Ok(image) if image.id == worker.reference.asset.image_id => {
                    match self.supervisor.accept_result(
                        worker.reference,
                        worker.reference.asset,
                        now_ms,
                    ) {
                        Ok(JobResultDisposition::Accepted(job)) => {
                            completions
                                .push(ClipboardImageReadCompletion::Completed { job, image });
                        }
                        Ok(JobResultDisposition::Rejected(terminal)) => {
                            completions.push(ClipboardImageReadCompletion::Discarded {
                                job_id: worker.reference.job_id,
                                owner: spec.owner,
                                terminal,
                            });
                        }
                        Err(error) => completions.push(ClipboardImageReadCompletion::Failed {
                            job_id: worker.reference.job_id,
                            owner: spec.owner,
                            error,
                        }),
                    }
                }
                Ok(_) => {
                    let _ = self.supervisor.fail(worker.reference.job_id);
                    completions.push(ClipboardImageReadCompletion::Failed {
                        job_id: worker.reference.job_id,
                        owner: spec.owner,
                        error: PinoraError::new(
                            ErrorCode::InvalidState,
                            "clipboard image identity mismatch",
                        ),
                    });
                }
                Err(error) => match self.supervisor.fail(worker.reference.job_id) {
                    Ok(JobState::Finished(JobTerminalState::Failed)) => {
                        completions.push(ClipboardImageReadCompletion::Failed {
                            job_id: worker.reference.job_id,
                            owner: spec.owner,
                            error,
                        });
                    }
                    Ok(JobState::Finished(terminal)) => {
                        completions.push(ClipboardImageReadCompletion::Discarded {
                            job_id: worker.reference.job_id,
                            owner: spec.owner,
                            terminal,
                        });
                    }
                    Ok(JobState::Running) => {}
                    Err(supervisor_error) => {
                        completions.push(ClipboardImageReadCompletion::Failed {
                            job_id: worker.reference.job_id,
                            owner: spec.owner,
                            error: supervisor_error,
                        })
                    }
                },
            }
        }
        let _ = reap_finished_workers(&mut self.workers);
        completions
    }
}

impl<R: ClipboardImageReader> Drop for ClipboardImageReadJobService<R> {
    fn drop(&mut self) {
        self.supervisor.cancel_all();
        let _ = wait_for_workers(&mut self.workers, Duration::from_millis(50));
    }
}

fn read_clipboard_png(cancellation: &JobCancellation) -> Result<Vec<u8>, PinoraError> {
    if std::env::var_os("PINORA_NO_SYSTEM_CLIPBOARD").is_some() {
        return Err(clipboard_failure("system clipboard is disabled"));
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(path) = which("wl-paste") {
            return read_command(&path, &["--type", "image/png"], cancellation);
        }
        if let Some(path) = which("xclip") {
            return read_command(
                &path,
                &["-selection", "clipboard", "-t", "image/png", "-o"],
                cancellation,
            );
        }
        Err(PinoraError::new(
            ErrorCode::CapabilityUnavailable,
            "no wl-paste/xclip image clipboard reader in PATH",
        ))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = cancellation;
        Err(PinoraError::new(
            ErrorCode::CapabilityUnavailable,
            "system clipboard image import is not implemented on this platform",
        ))
    }
}

#[cfg(target_os = "linux")]
fn read_command(
    program: &Path,
    args: &[&str],
    cancellation: &JobCancellation,
) -> Result<Vec<u8>, PinoraError> {
    let output = OwnedTempFile::create("clipboard-read")?;
    let output_file = output.open_for_write()?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(output_file))
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| clipboard_failure("spawn system clipboard reader failed"))?;
    let deadline = Instant::now() + CLIPBOARD_READ_TIMEOUT;
    loop {
        if cancellation.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(cancelled());
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return output.read_bounded(MAX_PNG_BYTES),
            Ok(Some(_)) => {
                return Err(clipboard_failure(
                    "system clipboard has no image/png payload",
                ));
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(clipboard_failure("system clipboard reader timed out"));
            }
            Ok(None) => thread::sleep(Duration::from_millis(5)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(clipboard_failure("wait system clipboard reader failed"));
            }
        }
    }
}

fn decode_clipboard_png(image_id: ImageId, bytes: &[u8]) -> Result<CaptureImage, PinoraError> {
    if bytes.is_empty() || bytes.len() > MAX_PNG_BYTES {
        return Err(clipboard_failure(
            "clipboard PNG payload is empty or too large",
        ));
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|_| clipboard_failure("clipboard image/png header is invalid"))?;
    let info = reader.info();
    let pixel_count = u64::from(info.width).saturating_mul(u64::from(info.height));
    if info.width == 0 || info.height == 0 || pixel_count > MAX_PIXELS {
        return Err(clipboard_failure("clipboard image dimensions exceed limit"));
    }
    let buffer_len = reader.output_buffer_size();
    if buffer_len > MAX_RGBA_BYTES {
        return Err(clipboard_failure(
            "clipboard image decoded payload exceeds limit",
        ));
    }
    let mut decoded = vec![0; buffer_len];
    let output = reader
        .next_frame(&mut decoded)
        .map_err(|_| clipboard_failure("clipboard image/png decode failed"))?;
    let rgba = normalize_to_rgba(output.color_type, &decoded[..output.buffer_size()])?;
    let size = PixelSize::new(output.width, output.height);
    let pixels = RgbaBuffer::new(size, rgba)
        .map_err(|_| clipboard_failure("clipboard image has invalid RGBA dimensions"))?;
    CaptureImage::new(
        image_id,
        pixels,
        PixelRect::new(0, 0, size.width, size.height),
        CaptureMetadata::new(DisplayId::new("pinora:clipboard"), 1.0, unix_time_ms()),
    )
    .map_err(|_| clipboard_failure("clipboard image is empty"))
}

fn normalize_to_rgba(color: png::ColorType, bytes: &[u8]) -> Result<Vec<u8>, PinoraError> {
    let channels = match color {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err(clipboard_failure("clipboard palette PNG was not expanded"));
        }
    };
    if !bytes.len().is_multiple_of(channels) {
        return Err(clipboard_failure("clipboard PNG channel layout is invalid"));
    }
    let mut out = Vec::with_capacity(bytes.len() / channels * 4);
    for pixel in bytes.chunks_exact(channels) {
        match color {
            png::ColorType::Grayscale => {
                out.extend_from_slice(&[pixel[0], pixel[0], pixel[0], 255])
            }
            png::ColorType::GrayscaleAlpha => {
                out.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]])
            }
            png::ColorType::Rgb => out.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]),
            png::ColorType::Rgba => out.extend_from_slice(pixel),
            png::ColorType::Indexed => unreachable!(),
        }
    }
    Ok(out)
}

#[cfg(target_os = "linux")]
struct OwnedTempFile {
    path: PathBuf,
}

#[cfg(target_os = "linux")]
impl OwnedTempFile {
    fn create(kind: &str) -> Result<Self, PinoraError> {
        for _ in 0..16 {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("pinora-{kind}-{}-{id}.tmp", std::process::id()));
            match OpenOptions::new().create_new(true).write(true).open(&path) {
                Ok(_) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(clipboard_failure("create clipboard temporary file failed")),
            }
        }
        Err(clipboard_failure(
            "create clipboard temporary file collision limit reached",
        ))
    }

    fn open_for_write(&self) -> Result<File, PinoraError> {
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)
            .map_err(|_| clipboard_failure("open clipboard temporary file failed"))
    }

    fn read_bounded(&self, limit: usize) -> Result<Vec<u8>, PinoraError> {
        let mut file = File::open(&self.path)
            .map_err(|_| clipboard_failure("read clipboard temporary file failed"))?;
        let mut bytes = Vec::new();
        file.by_ref()
            .take((limit as u64).saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| clipboard_failure("read system clipboard image failed"))?;
        if bytes.len() > limit {
            return Err(clipboard_failure(
                "system clipboard image exceeds byte limit",
            ));
        }
        Ok(bytes)
    }
}

#[cfg(target_os = "linux")]
impl Drop for OwnedTempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(target_os = "linux")]
fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    })
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn clipboard_failure(message: &'static str) -> PinoraError {
    PinoraError::new(ErrorCode::ClipboardFailed, message)
}

fn cancelled() -> PinoraError {
    PinoraError::new(ErrorCode::Cancelled, "clipboard image read cancelled")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn encoded_png(color: png::ColorType, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(pixels).expect("pixels");
        drop(writer);
        bytes
    }

    fn sample_image(image_id: ImageId) -> CaptureImage {
        let size = PixelSize::new(1, 1);
        CaptureImage::new(
            image_id,
            RgbaBuffer::solid(size, [10, 20, 30, 255]),
            PixelRect::new(0, 0, size.width, size.height),
            CaptureMetadata::new(DisplayId::new("clipboard-test"), 1.0, 0),
        )
        .expect("sample image")
    }

    fn spec(id: u64, image_id: ImageId) -> JobSpec {
        JobSpec::new(
            JobId::from_raw(id),
            pinora_core::CorrelationId::from_raw(id),
            pinora_core::AssetRef::initial(image_id),
            JobOwner::Clipboard(image_id),
            JobKind::Clipboard,
            u64::MAX,
        )
    }

    fn poll_until<R: ClipboardImageReader>(
        service: &mut ClipboardImageReadJobService<R>,
    ) -> Vec<ClipboardImageReadCompletion> {
        for _ in 0..50 {
            let completions = service.poll(0);
            if !completions.is_empty() {
                return completions;
            }
            thread::sleep(Duration::from_millis(2));
        }
        Vec::new()
    }

    struct SuccessReader;

    impl ClipboardImageReader for SuccessReader {
        fn read_image(
            &self,
            image_id: ImageId,
            _cancellation: &JobCancellation,
        ) -> Result<CaptureImage, PinoraError> {
            Ok(sample_image(image_id))
        }
    }

    struct WrongIdentityReader;

    impl ClipboardImageReader for WrongIdentityReader {
        fn read_image(
            &self,
            _image_id: ImageId,
            _cancellation: &JobCancellation,
        ) -> Result<CaptureImage, PinoraError> {
            Ok(sample_image(ImageId::from_raw(999)))
        }
    }

    struct WaitForCancellationReader(AtomicBool);

    impl ClipboardImageReader for WaitForCancellationReader {
        fn read_image(
            &self,
            _image_id: ImageId,
            cancellation: &JobCancellation,
        ) -> Result<CaptureImage, PinoraError> {
            while !cancellation.is_cancelled() {
                thread::yield_now();
            }
            self.0.store(true, Ordering::Relaxed);
            Err(cancelled())
        }
    }

    #[test]
    fn png_decode_normalizes_rgb_and_preserves_the_requested_identity() {
        let bytes = encoded_png(png::ColorType::Rgb, &[1, 2, 3, 4, 5, 6]);
        let image = decode_clipboard_png(ImageId::from_raw(42), &bytes).expect("decode RGB PNG");
        assert_eq!(image.id, ImageId::from_raw(42));
        assert_eq!(image.pixels.size, PixelSize::new(2, 1));
        assert_eq!(image.pixels.bytes, vec![1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn png_decode_rejects_non_png_bytes() {
        let error = decode_clipboard_png(ImageId::from_raw(1), b"not a png")
            .expect_err("must reject malformed clipboard payload");
        assert_eq!(error.code, ErrorCode::ClipboardFailed);
    }

    #[test]
    fn matching_clipboard_job_delivers_the_requested_image_identity() {
        let image_id = ImageId::from_raw(7);
        let mut service = ClipboardImageReadJobService::with_reader(SuccessReader);
        let ticket = service
            .start(spec(7, image_id))
            .expect("start clipboard read");

        let completions = poll_until(&mut service);

        assert!(matches!(
            completions.as_slice(),
            [ClipboardImageReadCompletion::Completed { job, image }]
                if job.id == ticket.id
                    && job.owner == JobOwner::Clipboard(image_id)
                    && job.asset.image_id == image_id
                    && image.id == image_id
        ));
    }

    #[test]
    fn mismatched_worker_image_identity_is_rejected() {
        let image_id = ImageId::from_raw(8);
        let mut service = ClipboardImageReadJobService::with_reader(WrongIdentityReader);
        let ticket = service
            .start(spec(8, image_id))
            .expect("start clipboard read");

        let completions = poll_until(&mut service);

        assert!(matches!(
            completions.as_slice(),
            [ClipboardImageReadCompletion::Failed { job_id, owner, error }]
                if *job_id == ticket.id
                    && *owner == JobOwner::Clipboard(image_id)
                    && error.code == ErrorCode::InvalidState
        ));
    }

    #[test]
    fn cancellation_waits_for_clipboard_worker_convergence() {
        let image_id = ImageId::from_raw(9);
        let reader = WaitForCancellationReader(AtomicBool::new(false));
        let mut service = ClipboardImageReadJobService::with_reader(reader);
        service
            .start(spec(9, image_id))
            .expect("start clipboard read");

        let outcome = service.cancel_all_and_wait(Duration::from_secs(1));

        assert_eq!(outcome.cancelled, 1);
        assert_eq!(outcome.joined, 1);
        assert_eq!(outcome.panicked, 0);
        assert_eq!(outcome.unfinished, 0);
    }

    #[test]
    fn incorrect_clipboard_job_owner_is_rejected_before_starting_a_worker() {
        let image_id = ImageId::from_raw(10);
        let mut service = ClipboardImageReadJobService::with_reader(SuccessReader);
        let invalid = JobSpec::new(
            JobId::from_raw(10),
            pinora_core::CorrelationId::from_raw(10),
            pinora_core::AssetRef::initial(image_id),
            JobOwner::History(image_id),
            JobKind::Clipboard,
            u64::MAX,
        );

        assert_eq!(
            service
                .start(invalid)
                .expect_err("wrong owner must be rejected")
                .code,
            ErrorCode::CommandRejected
        );
    }
}
