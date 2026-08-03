//! 受监督的文件导出与系统剪贴板应用服务。
//!
//! worker 只持有不可变图像/文本副本、目标路径、取消令牌和结果发送器。
//! 结果回到事件循环后，必须先经过 `JobSupervisor` 的 owner、generation、
//! 截止时间和终态门禁，服务不把像素或 OCR 全文放入任务元数据或完成事件。

use std::path::PathBuf;
use std::sync::{
    Arc,
    mpsc::{self, Receiver, Sender},
};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use pinora_core::{
    AssetRef, CaptureImage, ErrorCode, ExportImageFormat, JobId, JobKind, JobOwner, JobResultRef,
    JobSpec, JobTerminalState, PinoraError,
};

use crate::image_sink::{
    copy_png_to_system_clipboard_with_cancellation,
    copy_text_to_system_clipboard_with_cancellation, encode_png_bytes,
    save_image_file_with_cancellation,
};
use crate::job_supervisor::{
    AcceptedJobResult, JobCancellation, JobResultDisposition, JobState, JobSupervisor, JobTicket,
};
use crate::worker_lifecycle::{WorkerWaitOutcome, reap_finished_workers, wait_for_workers};

/// 导出/剪贴板 worker 的输入。图像和文本只在进程内传递，不进入 `JobSpec`。
#[derive(Debug)]
pub enum ExportJobInput {
    SaveImage {
        image: CaptureImage,
        path: PathBuf,
        format: ExportImageFormat,
        jpeg_quality: u8,
    },
    CopyImage {
        image: CaptureImage,
    },
    CopyText {
        text: String,
    },
}

impl ExportJobInput {
    pub fn kind(&self) -> JobKind {
        match self {
            Self::SaveImage { .. } => JobKind::Export,
            Self::CopyImage { .. } | Self::CopyText { .. } => JobKind::Clipboard,
        }
    }

    fn image_id(&self) -> Option<pinora_core::ImageId> {
        match self {
            Self::SaveImage { image, .. } | Self::CopyImage { image } => Some(image.id),
            Self::CopyText { .. } => None,
        }
    }

    fn validate_file_target(&self) -> Result<(), PinoraError> {
        let Self::SaveImage {
            path,
            format,
            jpeg_quality,
            ..
        } = self
        else {
            return Ok(());
        };
        if path.extension().and_then(|extension| extension.to_str())
            != Some(format.file_extension())
        {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                "export path extension does not match frozen format",
            ));
        }
        if *format == ExportImageFormat::Jpeg && !(1..=100).contains(jpeg_quality) {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                "JPEG quality is outside the supported range",
            ));
        }
        Ok(())
    }
}

/// 可替换的导出执行端口。测试可注入纯内存 runner，生产实现使用本地适配器。
pub trait ExportRunner: Send + Sync + 'static {
    fn execute(
        &self,
        input: &ExportJobInput,
        cancellation: &JobCancellation,
    ) -> Result<(), PinoraError>;
}

/// 使用本地文件编码和 Linux 系统剪贴板适配器的生产 runner。
#[derive(Debug, Default)]
pub struct LocalExportRunner;

impl ExportRunner for LocalExportRunner {
    fn execute(
        &self,
        input: &ExportJobInput,
        cancellation: &JobCancellation,
    ) -> Result<(), PinoraError> {
        ensure_not_cancelled(cancellation)?;
        match input {
            ExportJobInput::SaveImage {
                image,
                path,
                format,
                jpeg_quality,
            } => {
                save_image_file_with_cancellation(
                    image,
                    path,
                    *format,
                    *jpeg_quality,
                    cancellation,
                )?;
            }
            ExportJobInput::CopyImage { image } => {
                let png = encode_png_bytes(image)?;
                ensure_not_cancelled(cancellation)?;
                copy_png_to_system_clipboard_with_cancellation(&png, cancellation)
                    .map_err(|error| PinoraError::new(ErrorCode::Internal, error))?;
            }
            ExportJobInput::CopyText { text } => {
                copy_text_to_system_clipboard_with_cancellation(text, cancellation)
                    .map_err(|error| PinoraError::new(ErrorCode::Internal, error))?;
            }
        }
        ensure_not_cancelled(cancellation)
    }
}

fn ensure_not_cancelled(cancellation: &JobCancellation) -> Result<(), PinoraError> {
    if cancellation.is_cancelled() {
        Err(PinoraError::new(
            ErrorCode::Cancelled,
            "export worker cancelled",
        ))
    } else {
        Ok(())
    }
}

#[derive(Debug)]
struct WorkerResult {
    reference: JobResultRef,
    result: Result<(), PinoraError>,
}

/// 主线程处理导出/剪贴板 worker 结果时的唯一输出。
#[derive(Debug)]
pub enum ExportJobCompletion {
    Completed {
        job: AcceptedJobResult,
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

/// 将导出/剪贴板 worker 与任务监督器连接的应用服务。
pub struct ExportJobService<R: ExportRunner = LocalExportRunner> {
    supervisor: JobSupervisor,
    runner: Arc<R>,
    sender: Sender<WorkerResult>,
    receiver: Receiver<WorkerResult>,
    workers: Vec<JoinHandle<()>>,
}

impl ExportJobService<LocalExportRunner> {
    pub fn new() -> Self {
        Self::with_runner(LocalExportRunner)
    }
}

impl Default for ExportJobService<LocalExportRunner> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R> ExportJobService<R>
where
    R: ExportRunner,
{
    pub fn with_runner(runner: R) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            supervisor: JobSupervisor::new(),
            runner: Arc::new(runner),
            sender,
            receiver,
            workers: Vec::new(),
        }
    }

    /// 提交并启动一个导出/剪贴板 worker。
    pub fn start(
        &mut self,
        spec: JobSpec,
        input: ExportJobInput,
    ) -> Result<JobTicket, PinoraError> {
        input.validate_file_target()?;
        if spec.kind != input.kind() {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                format!(
                    "export input requires {:?}, got {:?}",
                    input.kind(),
                    spec.kind
                ),
            ));
        }
        if let Some(image_id) = input.image_id()
            && spec.asset.image_id != image_id
        {
            return Err(PinoraError::new(
                ErrorCode::InvalidState,
                "export input image does not match JobSpec asset",
            ));
        }

        let ticket = self.supervisor.submit(spec)?;
        let cancellation = ticket.cancellation();
        let sender = self.sender.clone();
        let runner = self.runner.clone();
        let reference = JobResultRef::new(ticket.id, spec.asset);
        let worker = thread::Builder::new()
            .name(format!("pinora-export-{}", ticket.id.raw()))
            .spawn(move || {
                let result = runner.execute(&input, &cancellation);
                let _ = sender.send(WorkerResult { reference, result });
            });
        let worker = worker.map_err(|error| {
            let _ = self.supervisor.fail(ticket.id);
            PinoraError::new(ErrorCode::Internal, format!("spawn export worker: {error}"))
        })?;
        self.workers.push(worker);
        Ok(ticket)
    }

    pub fn state(&self, id: JobId) -> Option<JobState> {
        self.supervisor.state(id)
    }

    pub fn close_owner(&mut self, owner: JobOwner) -> usize {
        self.supervisor.close_owner(owner)
    }

    /// 取消单个仍在运行的导出或剪贴板任务；调用方负责限定可取消的用户意图。
    pub fn cancel(&mut self, id: JobId) -> Result<JobState, PinoraError> {
        self.supervisor.cancel(id)
    }

    pub fn cancel_all(&mut self) -> usize {
        self.supervisor.cancel_all()
    }

    pub(crate) fn cancel_all_and_wait(&mut self, timeout: Duration) -> WorkerWaitOutcome {
        let cancelled = self.cancel_all();
        let mut outcome = wait_for_workers(&mut self.workers, timeout);
        outcome.cancelled = cancelled;
        outcome
    }

    /// 轮询 worker 结果，并依据当前 owner 的资产引用决定是否交付。
    pub fn poll<F>(&mut self, now_ms: u64, mut current_asset: F) -> Vec<ExportJobCompletion>
    where
        F: FnMut(JobId, JobOwner) -> Option<AssetRef>,
    {
        self.supervisor.expire_at(now_ms);
        let mut completions = Vec::new();
        while let Ok(worker) = self.receiver.try_recv() {
            let Some(spec) = self.supervisor.spec(worker.reference.job_id) else {
                continue;
            };
            match worker.result {
                Ok(()) => {
                    let Some(asset) = current_asset(worker.reference.job_id, spec.owner) else {
                        self.supervisor.close_owner(spec.owner);
                        completions.push(ExportJobCompletion::Discarded {
                            job_id: worker.reference.job_id,
                            owner: spec.owner,
                            terminal: JobTerminalState::OwnerClosed,
                        });
                        continue;
                    };
                    match self
                        .supervisor
                        .accept_result(worker.reference, asset, now_ms)
                    {
                        Ok(JobResultDisposition::Accepted(job)) => {
                            completions.push(ExportJobCompletion::Completed { job });
                        }
                        Ok(JobResultDisposition::Rejected(terminal)) => {
                            completions.push(ExportJobCompletion::Discarded {
                                job_id: worker.reference.job_id,
                                owner: spec.owner,
                                terminal,
                            });
                        }
                        Err(error) => completions.push(ExportJobCompletion::Failed {
                            job_id: worker.reference.job_id,
                            owner: spec.owner,
                            error,
                        }),
                    }
                }
                Err(error) => {
                    let Some(asset) = current_asset(worker.reference.job_id, spec.owner) else {
                        self.supervisor.close_owner(spec.owner);
                        completions.push(ExportJobCompletion::Discarded {
                            job_id: worker.reference.job_id,
                            owner: spec.owner,
                            terminal: JobTerminalState::OwnerClosed,
                        });
                        continue;
                    };
                    if asset != worker.reference.asset {
                        let _ = self.supervisor.fail(worker.reference.job_id);
                        completions.push(ExportJobCompletion::Discarded {
                            job_id: worker.reference.job_id,
                            owner: spec.owner,
                            terminal: JobTerminalState::StaleAsset,
                        });
                        continue;
                    }
                    match self.supervisor.fail(worker.reference.job_id) {
                        Ok(JobState::Finished(JobTerminalState::Failed)) => {
                            completions.push(ExportJobCompletion::Failed {
                                job_id: worker.reference.job_id,
                                owner: spec.owner,
                                error,
                            });
                        }
                        Ok(JobState::Finished(terminal)) => {
                            completions.push(ExportJobCompletion::Discarded {
                                job_id: worker.reference.job_id,
                                owner: spec.owner,
                                terminal,
                            });
                        }
                        Ok(JobState::Running) => {}
                        Err(supervisor_error) => completions.push(ExportJobCompletion::Failed {
                            job_id: worker.reference.job_id,
                            owner: spec.owner,
                            error: supervisor_error,
                        }),
                    }
                }
            }
        }
        let _ = reap_finished_workers(&mut self.workers);
        completions
    }
}

impl<R> Drop for ExportJobService<R>
where
    R: ExportRunner,
{
    fn drop(&mut self) {
        self.supervisor.cancel_all();
        let _ = wait_for_workers(&mut self.workers, Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;
    use pinora_core::{
        CaptureMetadata, CorrelationId, DisplayId, ImageId, PixelRect, PixelSize, RgbaBuffer,
        SessionId,
    };

    #[derive(Debug)]
    struct SuccessRunner;

    impl ExportRunner for SuccessRunner {
        fn execute(
            &self,
            _input: &ExportJobInput,
            _cancellation: &JobCancellation,
        ) -> Result<(), PinoraError> {
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    struct FrozenFileInputRunner {
        seen: Arc<Mutex<Option<(ExportImageFormat, u8)>>>,
    }

    impl ExportRunner for FrozenFileInputRunner {
        fn execute(
            &self,
            input: &ExportJobInput,
            _cancellation: &JobCancellation,
        ) -> Result<(), PinoraError> {
            let ExportJobInput::SaveImage {
                format,
                jpeg_quality,
                ..
            } = input
            else {
                return Err(PinoraError::new(
                    ErrorCode::InvalidState,
                    "expected frozen file export input",
                ));
            };
            *self.seen.lock().expect("runner lock") = Some((*format, *jpeg_quality));
            Ok(())
        }
    }

    #[derive(Debug)]
    struct FailureRunner;

    impl ExportRunner for FailureRunner {
        fn execute(
            &self,
            _input: &ExportJobInput,
            _cancellation: &JobCancellation,
        ) -> Result<(), PinoraError> {
            Err(PinoraError::new(ErrorCode::Internal, "fake export failed"))
        }
    }

    #[derive(Debug)]
    struct WaitForCancellationRunner;

    impl ExportRunner for WaitForCancellationRunner {
        fn execute(
            &self,
            _input: &ExportJobInput,
            cancellation: &JobCancellation,
        ) -> Result<(), PinoraError> {
            while !cancellation.is_cancelled() {
                thread::yield_now();
            }
            Err(PinoraError::new(
                ErrorCode::Cancelled,
                "fake export cancelled",
            ))
        }
    }

    fn sample_image(id: u64) -> CaptureImage {
        CaptureImage::new(
            ImageId::from_raw(id),
            RgbaBuffer::solid(PixelSize::new(4, 4), [255, 255, 255, 255]),
            PixelRect::new(0, 0, 4, 4),
            CaptureMetadata::new(DisplayId::new("test"), 1.0, 0),
        )
        .expect("sample image")
    }

    fn spec(id: u64, asset: AssetRef, kind: JobKind, deadline_at_ms: u64) -> JobSpec {
        JobSpec::new(
            JobId::from_raw(id),
            CorrelationId::from_raw(id),
            asset,
            JobOwner::Session(SessionId::from_raw(id)),
            kind,
            deadline_at_ms,
        )
    }

    fn poll_until<R, F>(
        service: &mut ExportJobService<R>,
        now_ms: u64,
        current_asset: F,
    ) -> Vec<ExportJobCompletion>
    where
        R: ExportRunner,
        F: FnMut(JobId, JobOwner) -> Option<AssetRef> + Copy,
    {
        for _ in 0..50 {
            let events = service.poll(now_ms, current_asset);
            if !events.is_empty() {
                return events;
            }
            thread::sleep(Duration::from_millis(2));
        }
        Vec::new()
    }

    #[test]
    fn accepts_file_image_and_text_inputs_for_matching_kinds() {
        let image = sample_image(1);
        let asset = AssetRef::initial(image.id);
        let mut service = ExportJobService::with_runner(SuccessRunner);
        let save = service
            .start(
                spec(1, asset, JobKind::Export, 100),
                ExportJobInput::SaveImage {
                    image: image.clone(),
                    path: PathBuf::from("/tmp/pinora-test.png"),
                    format: ExportImageFormat::Png,
                    jpeg_quality: 90,
                },
            )
            .expect("save start");
        let copy_image = service
            .start(
                spec(2, asset, JobKind::Clipboard, 100),
                ExportJobInput::CopyImage { image },
            )
            .expect("image copy start");
        let copy_text = service
            .start(
                spec(3, asset, JobKind::Clipboard, 100),
                ExportJobInput::CopyText {
                    text: "private text".into(),
                },
            )
            .expect("text copy start");

        let mut completions = Vec::new();
        for _ in 0..50 {
            completions.extend(service.poll(1, |_, _| Some(asset)));
            if completions.len() == 3 {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(completions.len(), 3);
        assert!(
            completions
                .iter()
                .all(|completion| matches!(completion, ExportJobCompletion::Completed { .. }))
        );
        assert_eq!(
            service.state(save.id),
            Some(JobState::Finished(JobTerminalState::Completed))
        );
        assert_eq!(
            service.state(copy_image.id),
            Some(JobState::Finished(JobTerminalState::Completed))
        );
        assert_eq!(
            service.state(copy_text.id),
            Some(JobState::Finished(JobTerminalState::Completed))
        );
    }

    #[test]
    fn worker_receives_the_format_and_quality_frozen_at_submission() {
        let image = sample_image(11);
        let asset = AssetRef::initial(image.id);
        let seen = Arc::new(Mutex::new(None));
        let mut service =
            ExportJobService::with_runner(FrozenFileInputRunner { seen: seen.clone() });

        let ticket = service
            .start(
                spec(11, asset, JobKind::Export, 100),
                ExportJobInput::SaveImage {
                    image,
                    path: PathBuf::from("/tmp/pinora-test.webp"),
                    format: ExportImageFormat::WebP,
                    jpeg_quality: 37,
                },
            )
            .expect("start frozen file export");
        let completions = poll_until(&mut service, 1, |_, _| Some(asset));

        assert!(matches!(
            completions.as_slice(),
            [ExportJobCompletion::Completed { job }] if job.id == ticket.id
        ));
        assert_eq!(
            *seen.lock().expect("runner lock"),
            Some((ExportImageFormat::WebP, 37))
        );
    }

    #[test]
    fn rejects_kind_or_image_mismatch_before_worker_starts() {
        let image = sample_image(2);
        let asset = AssetRef::initial(image.id);
        let mut service = ExportJobService::with_runner(SuccessRunner);
        assert_eq!(
            service
                .start(
                    spec(2, asset, JobKind::Clipboard, 100),
                    ExportJobInput::SaveImage {
                        image: image.clone(),
                        path: PathBuf::from("/tmp/pinora-test.png"),
                        format: ExportImageFormat::Png,
                        jpeg_quality: 90,
                    },
                )
                .unwrap_err()
                .code,
            ErrorCode::CommandRejected
        );
        let other = sample_image(3);
        assert_eq!(
            service
                .start(
                    spec(3, asset, JobKind::Export, 100),
                    ExportJobInput::SaveImage {
                        image: other,
                        path: PathBuf::from("/tmp/pinora-test.png"),
                        format: ExportImageFormat::Png,
                        jpeg_quality: 90,
                    },
                )
                .unwrap_err()
                .code,
            ErrorCode::InvalidState
        );
    }

    #[test]
    fn rejects_mismatched_file_extension_or_invalid_jpeg_quality_before_worker_starts() {
        let image = sample_image(12);
        let asset = AssetRef::initial(image.id);
        let mut service = ExportJobService::with_runner(SuccessRunner);

        let extension_error = service
            .start(
                spec(12, asset, JobKind::Export, 100),
                ExportJobInput::SaveImage {
                    image: image.clone(),
                    path: PathBuf::from("/tmp/pinora-test.png"),
                    format: ExportImageFormat::WebP,
                    jpeg_quality: 90,
                },
            )
            .expect_err("mismatched extension must be rejected");
        assert_eq!(extension_error.code, ErrorCode::CommandRejected);

        let quality_error = service
            .start(
                spec(13, asset, JobKind::Export, 100),
                ExportJobInput::SaveImage {
                    image,
                    path: PathBuf::from("/tmp/pinora-test.jpg"),
                    format: ExportImageFormat::Jpeg,
                    jpeg_quality: 0,
                },
            )
            .expect_err("invalid JPEG quality must be rejected");
        assert_eq!(quality_error.code, ErrorCode::CommandRejected);
    }

    #[test]
    fn runner_failure_is_reported_without_accepting_result() {
        let image = sample_image(4);
        let asset = AssetRef::initial(image.id);
        let mut service = ExportJobService::with_runner(FailureRunner);
        let ticket = service
            .start(
                spec(4, asset, JobKind::Clipboard, 100),
                ExportJobInput::CopyImage { image },
            )
            .expect("start");
        let completions = poll_until(&mut service, 1, |_, _| Some(asset));
        assert!(matches!(
            completions.as_slice(),
            [ExportJobCompletion::Failed { job_id, error, .. }]
                if *job_id == ticket.id && error.code == ErrorCode::Internal
        ));
        assert_eq!(
            service.state(ticket.id),
            Some(JobState::Finished(JobTerminalState::Failed))
        );
    }

    #[test]
    fn closed_owner_discards_late_result() {
        let image = sample_image(5);
        let asset = AssetRef::initial(image.id);
        let job = spec(5, asset, JobKind::Export, 100);
        let owner = job.owner;
        let mut service = ExportJobService::with_runner(SuccessRunner);
        let ticket = service
            .start(
                job,
                ExportJobInput::SaveImage {
                    image,
                    path: PathBuf::from("/tmp/pinora-test.png"),
                    format: ExportImageFormat::Png,
                    jpeg_quality: 90,
                },
            )
            .expect("start");
        assert_eq!(service.close_owner(owner), 1);
        let completions = poll_until(&mut service, 1, |_, _| Some(asset));
        assert!(matches!(
            completions.as_slice(),
            [ExportJobCompletion::Discarded { job_id, terminal, .. }]
                if *job_id == ticket.id && *terminal == JobTerminalState::OwnerClosed
        ));
    }

    #[test]
    fn timeout_discards_worker_result() {
        let image = sample_image(6);
        let asset = AssetRef::initial(image.id);
        let mut service = ExportJobService::with_runner(WaitForCancellationRunner);
        let ticket = service
            .start(
                spec(6, asset, JobKind::Clipboard, 10),
                ExportJobInput::CopyText {
                    text: "wait".into(),
                },
            )
            .expect("start");
        let completions = poll_until(&mut service, 10, |_, _| Some(asset));
        assert!(matches!(
            completions.as_slice(),
            [ExportJobCompletion::Discarded { job_id, terminal, .. }]
                if *job_id == ticket.id && *terminal == JobTerminalState::TimedOut
        ));
    }

    #[test]
    fn cancellation_waits_for_worker_convergence() {
        let image = sample_image(8);
        let asset = AssetRef::initial(image.id);
        let mut service = ExportJobService::with_runner(WaitForCancellationRunner);
        let ticket = service
            .start(
                spec(8, asset, JobKind::Clipboard, 100),
                ExportJobInput::CopyText {
                    text: "wait".into(),
                },
            )
            .expect("start");

        let outcome = service.cancel_all_and_wait(Duration::from_secs(1));

        assert_eq!(outcome.cancelled, 1);
        assert_eq!(outcome.joined, 1);
        assert_eq!(outcome.panicked, 0);
        assert_eq!(outcome.unfinished, 0);
        assert_eq!(
            service.state(ticket.id),
            Some(JobState::Finished(JobTerminalState::Cancelled))
        );
    }

    #[test]
    fn single_cancellation_discards_only_that_workers_result() {
        let image = sample_image(81);
        let asset = AssetRef::initial(image.id);
        let mut service = ExportJobService::with_runner(WaitForCancellationRunner);
        let ticket = service
            .start(
                spec(81, asset, JobKind::Export, 100),
                ExportJobInput::SaveImage {
                    image,
                    path: PathBuf::from("/tmp/pinora-test.png"),
                    format: ExportImageFormat::Png,
                    jpeg_quality: 90,
                },
            )
            .expect("start");

        assert_eq!(service.state(ticket.id), Some(JobState::Running));
        assert_eq!(
            service.cancel(ticket.id),
            Ok(JobState::Finished(JobTerminalState::Cancelled))
        );
        assert_eq!(
            service.cancel(ticket.id),
            Ok(JobState::Finished(JobTerminalState::Cancelled))
        );

        let completions = poll_until(&mut service, 1, |_, _| Some(asset));
        assert!(matches!(
            completions.as_slice(),
            [ExportJobCompletion::Discarded { job_id, terminal, .. }]
                if *job_id == ticket.id && *terminal == JobTerminalState::Cancelled
        ));
    }

    #[test]
    fn changed_asset_generation_discards_result() {
        let image = sample_image(7);
        let submitted = AssetRef::initial(image.id);
        let current = submitted.advance().expect("advance generation");
        let mut service = ExportJobService::with_runner(SuccessRunner);
        let ticket = service
            .start(
                spec(7, submitted, JobKind::Export, 100),
                ExportJobInput::SaveImage {
                    image,
                    path: PathBuf::from("/tmp/pinora-test.png"),
                    format: ExportImageFormat::Png,
                    jpeg_quality: 90,
                },
            )
            .expect("start");
        let completions = poll_until(&mut service, 1, |_, _| Some(current));
        assert!(matches!(
            completions.as_slice(),
            [ExportJobCompletion::Discarded { job_id, terminal, .. }]
                if *job_id == ticket.id && *terminal == JobTerminalState::StaleAsset
        ));
    }

    #[test]
    fn changed_asset_generation_discards_failure() {
        let image = sample_image(9);
        let submitted = AssetRef::initial(image.id);
        let current = submitted.advance().expect("advance generation");
        let mut service = ExportJobService::with_runner(FailureRunner);
        let ticket = service
            .start(
                spec(9, submitted, JobKind::Clipboard, 100),
                ExportJobInput::CopyImage { image },
            )
            .expect("start");
        let completions = poll_until(&mut service, 1, |_, _| Some(current));

        assert!(matches!(
            completions.as_slice(),
            [ExportJobCompletion::Discarded { job_id, terminal, .. }]
                if *job_id == ticket.id && *terminal == JobTerminalState::StaleAsset
        ));
    }

    #[test]
    fn missing_owner_discards_failure() {
        let image = sample_image(10);
        let asset = AssetRef::initial(image.id);
        let mut service = ExportJobService::with_runner(FailureRunner);
        let ticket = service
            .start(
                spec(10, asset, JobKind::Clipboard, 100),
                ExportJobInput::CopyImage { image },
            )
            .expect("start");
        let completions = poll_until(&mut service, 1, |_, _| None);

        assert!(matches!(
            completions.as_slice(),
            [ExportJobCompletion::Discarded { job_id, terminal, .. }]
                if *job_id == ticket.id && *terminal == JobTerminalState::OwnerClosed
        ));
    }
}
