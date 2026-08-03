//! 受监督的历史图像读取应用服务。
//!
//! 历史 PNG 的路径、摘要和像素校验仍由 `history_export` 负责。本模块只把读取
//! 放到单个 worker 中，并在结果回到事件循环后按历史条目资产引用重新确认。

use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, Sender},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pinora_core::{
    AssetRef, CaptureImage, ErrorCode, HistoryEntry, JobId, JobKind, JobOwner, JobResultRef,
    JobSpec, JobTerminalState, PinoraError, PixelSize,
};

use crate::history_export::load_history_image;
use pinora_capture::{rgba_to_xrgb, rgba_to_xrgb_and_dim};
use pinora_jobs::{
    AcceptedJobResult, JobCancellation, JobResultDisposition, JobState, JobSupervisor, JobTicket,
};
use pinora_jobs::{WorkerWaitOutcome, reap_finished_workers, wait_for_workers};

/// 历史读取 worker 的不可变输入。
#[derive(Debug, Clone)]
pub struct HistoryLoadInput {
    pub export_dir: PathBuf,
    pub entry: HistoryEntry,
    pub preparation: HistoryLoadPreparation,
}

/// 由历史加载 worker 预先生成的最小显示材料。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryLoadPreparation {
    Preview,
    Pin,
    Editor,
}

/// 已经通过历史文件校验且按消费意图完成像素准备的结果。
#[derive(Debug)]
pub enum HistoryLoadPayload {
    Preview {
        size: PixelSize,
        pixels_xrgb: Vec<u32>,
    },
    Pin {
        image: CaptureImage,
        pixels_xrgb: Vec<u32>,
    },
    Editor {
        image: CaptureImage,
        base: Vec<u32>,
        dimmed: Vec<u32>,
    },
}

impl HistoryLoadPayload {
    pub const fn preparation(&self) -> HistoryLoadPreparation {
        match self {
            Self::Preview { .. } => HistoryLoadPreparation::Preview,
            Self::Pin { .. } => HistoryLoadPreparation::Pin,
            Self::Editor { .. } => HistoryLoadPreparation::Editor,
        }
    }
}

/// 可替换的历史读取执行端口。生产实现只读受管 PNG，测试可注入内存 runner。
pub trait HistoryLoadRunner: Send + Sync + 'static {
    fn load(
        &self,
        export_dir: &Path,
        entry: &HistoryEntry,
        cancellation: &JobCancellation,
    ) -> Result<CaptureImage, PinoraError>;
}

/// 默认本地历史读取 runner。
#[derive(Debug, Default)]
pub struct LocalHistoryLoadRunner;

impl HistoryLoadRunner for LocalHistoryLoadRunner {
    fn load(
        &self,
        export_dir: &Path,
        entry: &HistoryEntry,
        cancellation: &JobCancellation,
    ) -> Result<CaptureImage, PinoraError> {
        ensure_not_cancelled(cancellation)?;
        let image = load_history_image(export_dir, entry)
            .map_err(|error| PinoraError::new(ErrorCode::Internal, error))?;
        ensure_not_cancelled(cancellation)?;
        Ok(image)
    }
}

fn ensure_not_cancelled(cancellation: &JobCancellation) -> Result<(), PinoraError> {
    if cancellation.is_cancelled() {
        Err(PinoraError::new(
            ErrorCode::Cancelled,
            "history load worker cancelled",
        ))
    } else {
        Ok(())
    }
}

#[derive(Debug)]
struct WorkerResult {
    reference: JobResultRef,
    result: Result<HistoryLoadPayload, PinoraError>,
}

/// 主线程处理历史读取 worker 的唯一输出。
#[derive(Debug)]
pub enum HistoryLoadCompletion {
    Completed {
        job: AcceptedJobResult,
        payload: HistoryLoadPayload,
    },
    Failed {
        job_id: JobId,
        owner: JobOwner,
        error: PinoraError,
    },
    Discarded {
        job_id: JobId,
        terminal: JobTerminalState,
    },
}

fn prepare_history_payload(
    image: CaptureImage,
    preparation: HistoryLoadPreparation,
    cancellation: &JobCancellation,
) -> Result<HistoryLoadPayload, PinoraError> {
    ensure_not_cancelled(cancellation)?;
    let payload = match preparation {
        HistoryLoadPreparation::Preview => HistoryLoadPayload::Preview {
            size: image.pixels.size,
            pixels_xrgb: rgba_to_xrgb(&image.pixels.bytes),
        },
        HistoryLoadPreparation::Pin => HistoryLoadPayload::Pin {
            pixels_xrgb: rgba_to_xrgb(&image.pixels.bytes),
            image,
        },
        HistoryLoadPreparation::Editor => {
            let (base, dimmed) = rgba_to_xrgb_and_dim(&image.pixels.bytes);
            HistoryLoadPayload::Editor {
                image,
                base,
                dimmed,
            }
        }
    };
    ensure_not_cancelled(cancellation)?;
    Ok(payload)
}

/// 历史窗口只允许一个实际文件读取 worker，避免快速搜索或切换时堆积大图解码。
pub struct HistoryLoadJobService<R: HistoryLoadRunner = LocalHistoryLoadRunner> {
    supervisor: JobSupervisor,
    runner: Arc<R>,
    sender: Sender<WorkerResult>,
    receiver: Receiver<WorkerResult>,
    workers: Vec<JoinHandle<()>>,
}

impl HistoryLoadJobService<LocalHistoryLoadRunner> {
    pub fn new() -> Self {
        Self::with_runner(LocalHistoryLoadRunner)
    }
}

impl Default for HistoryLoadJobService<LocalHistoryLoadRunner> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R> HistoryLoadJobService<R>
where
    R: HistoryLoadRunner,
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

    /// 启动一次历史读取。尚未收敛的取消 worker 存在时拒绝新启动，由调用方保留最新请求。
    pub fn start(
        &mut self,
        spec: JobSpec,
        input: HistoryLoadInput,
    ) -> Result<JobTicket, PinoraError> {
        let _ = reap_finished_workers(&mut self.workers);
        if !self.workers.is_empty() {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                "history load worker is still active",
            ));
        }
        if spec.kind != JobKind::HistoryLoad {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                "history load service accepts only JobKind::HistoryLoad",
            ));
        }
        let entry_asset = AssetRef::new(input.entry.image_id, input.entry.generation);
        if spec.owner != JobOwner::History(input.entry.image_id) || spec.asset != entry_asset {
            return Err(PinoraError::new(
                ErrorCode::InvalidState,
                "history load JobSpec does not match history entry identity",
            ));
        }

        let ticket = self.supervisor.submit(spec)?;
        let cancellation = ticket.cancellation();
        let sender = self.sender.clone();
        let runner = self.runner.clone();
        let reference = JobResultRef::new(ticket.id, spec.asset);
        let worker = thread::Builder::new()
            .name(format!("pinora-history-load-{}", ticket.id.raw()))
            .spawn(move || {
                let result = runner
                    .load(&input.export_dir, &input.entry, &cancellation)
                    .and_then(|image| {
                        prepare_history_payload(image, input.preparation, &cancellation)
                    });
                let _ = sender.send(WorkerResult { reference, result });
            });
        let worker = worker.map_err(|error| {
            let _ = self.supervisor.fail(ticket.id);
            PinoraError::new(
                ErrorCode::Internal,
                format!("spawn history load worker: {error}"),
            )
        })?;
        self.workers.push(worker);
        Ok(ticket)
    }

    pub fn is_idle(&mut self) -> bool {
        let _ = reap_finished_workers(&mut self.workers);
        self.workers.is_empty()
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

    /// 轮询并仅交付仍属于当前历史选择的读取结果。
    pub fn poll<F>(&mut self, now_ms: u64, mut current_asset: F) -> Vec<HistoryLoadCompletion>
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
                Ok(payload) => {
                    let disposition = match current_asset(worker.reference.job_id, spec.owner) {
                        Some(asset) => {
                            self.supervisor
                                .accept_result(worker.reference, asset, now_ms)
                        }
                        None => self
                            .supervisor
                            .cancel(worker.reference.job_id)
                            .map(|state| match state {
                                JobState::Finished(terminal) => {
                                    JobResultDisposition::Rejected(terminal)
                                }
                                JobState::Running => {
                                    JobResultDisposition::Rejected(JobTerminalState::Cancelled)
                                }
                            }),
                    };
                    match disposition {
                        Ok(JobResultDisposition::Accepted(job)) => {
                            completions.push(HistoryLoadCompletion::Completed { job, payload });
                        }
                        Ok(JobResultDisposition::Rejected(terminal)) => {
                            completions.push(HistoryLoadCompletion::Discarded {
                                job_id: worker.reference.job_id,
                                terminal,
                            });
                        }
                        Err(error) => completions.push(HistoryLoadCompletion::Failed {
                            job_id: worker.reference.job_id,
                            owner: spec.owner,
                            error,
                        }),
                    }
                }
                Err(error) => match self.supervisor.fail(worker.reference.job_id) {
                    Ok(JobState::Finished(JobTerminalState::Failed)) => {
                        completions.push(HistoryLoadCompletion::Failed {
                            job_id: worker.reference.job_id,
                            owner: spec.owner,
                            error,
                        });
                    }
                    Ok(JobState::Finished(terminal)) => {
                        completions.push(HistoryLoadCompletion::Discarded {
                            job_id: worker.reference.job_id,
                            terminal,
                        });
                    }
                    Ok(JobState::Running) => {}
                    Err(supervisor_error) => completions.push(HistoryLoadCompletion::Failed {
                        job_id: worker.reference.job_id,
                        owner: spec.owner,
                        error: supervisor_error,
                    }),
                },
            }
        }
        let _ = reap_finished_workers(&mut self.workers);
        completions
    }
}

impl<R> Drop for HistoryLoadJobService<R>
where
    R: HistoryLoadRunner,
{
    fn drop(&mut self) {
        self.supervisor.cancel_all();
        let _ = wait_for_workers(&mut self.workers, Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use pinora_core::{
        AssetGeneration, CaptureMetadata, ContentDigest, CorrelationId, DisplayId, HistoryEntry,
        HistoryEntrySpec, HistoryOcrState, ImageId, JobId, PixelRect, PixelSize, RgbaBuffer,
    };

    #[derive(Debug)]
    struct SuccessRunner;

    impl HistoryLoadRunner for SuccessRunner {
        fn load(
            &self,
            _export_dir: &Path,
            _entry: &HistoryEntry,
            _cancellation: &JobCancellation,
        ) -> Result<CaptureImage, PinoraError> {
            Ok(sample_image(900))
        }
    }

    #[derive(Debug)]
    struct FailureRunner;

    impl HistoryLoadRunner for FailureRunner {
        fn load(
            &self,
            _export_dir: &Path,
            _entry: &HistoryEntry,
            _cancellation: &JobCancellation,
        ) -> Result<CaptureImage, PinoraError> {
            Err(PinoraError::new(
                ErrorCode::Internal,
                "fake history load failed",
            ))
        }
    }

    #[derive(Debug)]
    struct WaitForCancellationRunner(AtomicBool);

    impl HistoryLoadRunner for WaitForCancellationRunner {
        fn load(
            &self,
            _export_dir: &Path,
            _entry: &HistoryEntry,
            cancellation: &JobCancellation,
        ) -> Result<CaptureImage, PinoraError> {
            self.0.store(true, Ordering::Release);
            while !cancellation.is_cancelled() {
                thread::yield_now();
            }
            Err(PinoraError::new(
                ErrorCode::Cancelled,
                "fake history load cancelled",
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

    fn entry(id: u64) -> HistoryEntry {
        HistoryEntry::new(HistoryEntrySpec {
            image_id: ImageId::from_raw(id),
            generation: AssetGeneration::INITIAL,
            created_at_ms: id,
            display: DisplayId::new("test"),
            source_rect: PixelRect::new(0, 0, 4, 4),
            file_name: format!("{id}.png"),
            byte_len: 1,
            digest: ContentDigest::of(b"x"),
            ocr: HistoryOcrState::Unknown,
        })
        .expect("history entry")
    }

    fn spec(id: u64, entry: &HistoryEntry, kind: JobKind) -> JobSpec {
        JobSpec::new(
            JobId::from_raw(id),
            CorrelationId::from_raw(id),
            AssetRef::new(entry.image_id, entry.generation),
            JobOwner::History(entry.image_id),
            kind,
            100,
        )
    }

    fn input(entry: HistoryEntry) -> HistoryLoadInput {
        input_with_preparation(entry, HistoryLoadPreparation::Preview)
    }

    fn input_with_preparation(
        entry: HistoryEntry,
        preparation: HistoryLoadPreparation,
    ) -> HistoryLoadInput {
        HistoryLoadInput {
            export_dir: PathBuf::from("history-test"),
            entry,
            preparation,
        }
    }

    fn poll_until<R, F>(
        service: &mut HistoryLoadJobService<R>,
        current_asset: F,
    ) -> Vec<HistoryLoadCompletion>
    where
        R: HistoryLoadRunner,
        F: FnMut(JobId, JobOwner) -> Option<AssetRef> + Copy,
    {
        for _ in 0..50 {
            let events = service.poll(1, current_asset);
            if !events.is_empty() {
                return events;
            }
            thread::sleep(Duration::from_millis(2));
        }
        Vec::new()
    }

    #[test]
    fn matching_entry_delivers_prepared_preview_to_main_thread() {
        let entry = entry(1);
        let asset = AssetRef::new(entry.image_id, entry.generation);
        let mut service = HistoryLoadJobService::with_runner(SuccessRunner);
        let ticket = service
            .start(spec(1, &entry, JobKind::HistoryLoad), input(entry))
            .expect("start");

        let completions = poll_until(&mut service, |job_id, owner| {
            (job_id == ticket.id && owner == JobOwner::History(asset.image_id)).then_some(asset)
        });

        assert!(matches!(
            completions.as_slice(),
            [HistoryLoadCompletion::Completed {
                job,
                payload: HistoryLoadPayload::Preview { size, pixels_xrgb },
            }]
                if job.id == ticket.id
                    && job.asset == asset
                    && *size == PixelSize::new(4, 4)
                    && pixels_xrgb == &vec![0x00ff_ffff; 16]
        ));
    }

    #[test]
    fn pin_preparation_keeps_image_and_precomputed_xrgb() {
        let entry = entry(11);
        let asset = AssetRef::new(entry.image_id, entry.generation);
        let mut service = HistoryLoadJobService::with_runner(SuccessRunner);
        let ticket = service
            .start(
                spec(11, &entry, JobKind::HistoryLoad),
                input_with_preparation(entry, HistoryLoadPreparation::Pin),
            )
            .expect("start");

        let completions = poll_until(&mut service, |job_id, owner| {
            (job_id == ticket.id && owner == JobOwner::History(asset.image_id)).then_some(asset)
        });

        assert!(matches!(
            completions.as_slice(),
            [HistoryLoadCompletion::Completed {
                payload: HistoryLoadPayload::Pin { image, pixels_xrgb },
                ..
            }] if image.pixels.size == PixelSize::new(4, 4)
                && pixels_xrgb == &vec![0x00ff_ffff; 16]
        ));
    }

    #[test]
    fn editor_preparation_keeps_image_base_and_dimmed_pixels() {
        let entry = entry(12);
        let asset = AssetRef::new(entry.image_id, entry.generation);
        let mut service = HistoryLoadJobService::with_runner(SuccessRunner);
        let ticket = service
            .start(
                spec(12, &entry, JobKind::HistoryLoad),
                input_with_preparation(entry, HistoryLoadPreparation::Editor),
            )
            .expect("start");

        let completions = poll_until(&mut service, |job_id, owner| {
            (job_id == ticket.id && owner == JobOwner::History(asset.image_id)).then_some(asset)
        });

        assert!(matches!(
            completions.as_slice(),
            [HistoryLoadCompletion::Completed {
                payload: HistoryLoadPayload::Editor { image, base, dimmed },
                ..
            }] if image.pixels.size == PixelSize::new(4, 4)
                && base == &vec![0x00ff_ffff; 16]
                && dimmed == &vec![0x008c_8c8c; 16]
        ));
    }

    #[test]
    fn payload_preparation_matches_its_variant() {
        let preview = HistoryLoadPayload::Preview {
            size: PixelSize::new(1, 1),
            pixels_xrgb: vec![0],
        };
        let pin = HistoryLoadPayload::Pin {
            image: sample_image(13),
            pixels_xrgb: vec![0],
        };
        let editor = HistoryLoadPayload::Editor {
            image: sample_image(14),
            base: vec![0],
            dimmed: vec![0],
        };

        assert_eq!(preview.preparation(), HistoryLoadPreparation::Preview);
        assert_eq!(pin.preparation(), HistoryLoadPreparation::Pin);
        assert_eq!(editor.preparation(), HistoryLoadPreparation::Editor);
    }

    #[test]
    fn runner_failure_is_delivered_without_accepting_result() {
        let entry = entry(2);
        let mut service = HistoryLoadJobService::with_runner(FailureRunner);
        let ticket = service
            .start(spec(2, &entry, JobKind::HistoryLoad), input(entry))
            .expect("start");

        let completions = poll_until(&mut service, |_, _| {
            Some(AssetRef::new(
                ImageId::from_raw(2),
                AssetGeneration::INITIAL,
            ))
        });

        assert!(matches!(
            completions.as_slice(),
            [HistoryLoadCompletion::Failed { job_id, error, .. }]
                if *job_id == ticket.id && error.code == ErrorCode::Internal
        ));
    }

    #[test]
    fn missing_current_selection_discards_completed_worker_result() {
        let entry = entry(3);
        let mut service = HistoryLoadJobService::with_runner(SuccessRunner);
        let ticket = service
            .start(spec(3, &entry, JobKind::HistoryLoad), input(entry))
            .expect("start");

        let completions = poll_until(&mut service, |_, _| None);

        assert!(matches!(
            completions.as_slice(),
            [HistoryLoadCompletion::Discarded { job_id, terminal }]
                if *job_id == ticket.id && *terminal == JobTerminalState::Cancelled
        ));
    }

    #[test]
    fn changed_entry_generation_discards_late_result() {
        let entry = entry(4);
        let current = AssetRef::new(entry.image_id, entry.generation)
            .advance()
            .expect("advance generation");
        let mut service = HistoryLoadJobService::with_runner(SuccessRunner);
        let ticket = service
            .start(spec(4, &entry, JobKind::HistoryLoad), input(entry))
            .expect("start");

        let completions = poll_until(&mut service, |_, _| Some(current));

        assert!(matches!(
            completions.as_slice(),
            [HistoryLoadCompletion::Discarded { job_id, terminal }]
                if *job_id == ticket.id && *terminal == JobTerminalState::StaleAsset
        ));
    }

    #[test]
    fn cancellation_waits_for_single_worker_convergence() {
        let entry = entry(5);
        let mut service =
            HistoryLoadJobService::with_runner(WaitForCancellationRunner(AtomicBool::new(false)));
        service
            .start(spec(5, &entry, JobKind::HistoryLoad), input(entry))
            .expect("start");

        let outcome = service.cancel_all_and_wait(Duration::from_secs(1));

        assert_eq!(outcome.cancelled, 1);
        assert_eq!(outcome.joined, 1);
        assert_eq!(outcome.panicked, 0);
        assert_eq!(outcome.unfinished, 0);
    }

    #[test]
    fn rejects_new_load_until_cancelled_worker_has_converged() {
        let first = entry(7);
        let second = entry(8);
        let mut service =
            HistoryLoadJobService::with_runner(WaitForCancellationRunner(AtomicBool::new(false)));
        service
            .start(spec(7, &first, JobKind::HistoryLoad), input(first))
            .expect("start first");

        assert_eq!(
            service
                .start(spec(8, &second, JobKind::HistoryLoad), input(second))
                .expect_err("second concurrent load must wait")
                .code,
            ErrorCode::CommandRejected
        );

        let outcome = service.cancel_all_and_wait(Duration::from_secs(1));
        assert_eq!(outcome.joined, 1);
        assert_eq!(outcome.unfinished, 0);
    }

    #[test]
    fn rejects_non_history_job_before_worker_starts() {
        let entry = entry(6);
        let mut service = HistoryLoadJobService::with_runner(SuccessRunner);

        assert_eq!(
            service
                .start(spec(6, &entry, JobKind::Ocr), input(entry))
                .expect_err("wrong job kind must fail")
                .code,
            ErrorCode::CommandRejected
        );
    }
}
