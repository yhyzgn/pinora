//! 受监督 OCR 应用服务。
//!
//! worker 只持有图像副本、协作式取消令牌和结果发送器。主线程轮询结果时，服务
//! 通过 `JobSupervisor` 重新检查 owner、截止时间和资产 generation，避免陈旧
//! 结果更新已关闭的窗口或已变更的资产。

use std::sync::{
    Arc,
    mpsc::{self, Receiver, Sender},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pinora_core::{
    AssetRef, CaptureImage, ErrorCode, JobId, JobKind, JobOwner, JobResultRef, JobSpec,
    JobTerminalState, OcrLanguage, OcrResult, PinoraError,
};

use crate::job_supervisor::{
    AcceptedJobResult, JobCancellation, JobResultDisposition, JobState, JobSupervisor, JobTicket,
};
use crate::ocr::recognize_image_with_cancellation;
use crate::worker_lifecycle::{WorkerWaitOutcome, reap_finished_workers, wait_for_workers};

/// 可替换的 OCR 执行端口。生产实现调用本地 Tesseract，测试可注入纯内存 runner。
pub trait OcrRunner: Send + Sync + 'static {
    fn recognize(
        &self,
        image: &CaptureImage,
        cancellation: &JobCancellation,
    ) -> Result<OcrResult, PinoraError>;

    /// 接收提交时冻结的预设。既有 runner 未覆盖时维持自动语言行为。
    fn recognize_with_language(
        &self,
        image: &CaptureImage,
        language: OcrLanguage,
        cancellation: &JobCancellation,
    ) -> Result<OcrResult, PinoraError> {
        let _ = language;
        self.recognize(image, cancellation)
    }
}

/// 默认的本地 OCR runner。
#[derive(Debug, Default)]
pub struct LocalOcrRunner;

impl OcrRunner for LocalOcrRunner {
    fn recognize(
        &self,
        image: &CaptureImage,
        cancellation: &JobCancellation,
    ) -> Result<OcrResult, PinoraError> {
        recognize_image_with_cancellation(image, cancellation)
    }

    fn recognize_with_language(
        &self,
        image: &CaptureImage,
        language: OcrLanguage,
        cancellation: &JobCancellation,
    ) -> Result<OcrResult, PinoraError> {
        crate::ocr::recognize_image_with_language(image, language, cancellation)
    }
}

#[derive(Debug)]
struct WorkerResult {
    reference: JobResultRef,
    result: Result<OcrResult, PinoraError>,
}

/// 主线程处理 OCR worker 结果时的唯一输出。
#[derive(Debug)]
pub enum OcrJobCompletion {
    Completed {
        job: AcceptedJobResult,
        result: OcrResult,
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

/// 将 OCR worker 与任务监督器连接的应用服务。
pub struct OcrJobService<R: OcrRunner = LocalOcrRunner> {
    supervisor: JobSupervisor,
    runner: Arc<R>,
    sender: Sender<WorkerResult>,
    receiver: Receiver<WorkerResult>,
    workers: Vec<JoinHandle<()>>,
}

impl OcrJobService<LocalOcrRunner> {
    pub fn new() -> Self {
        Self::with_runner(LocalOcrRunner)
    }
}

impl Default for OcrJobService<LocalOcrRunner> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R> OcrJobService<R>
where
    R: OcrRunner,
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

    /// 提交并启动一个 OCR worker。worker 的输出不会直接触碰 UI 或应用状态。
    pub fn start(&mut self, spec: JobSpec, image: CaptureImage) -> Result<JobTicket, PinoraError> {
        self.start_with_language(spec, image, OcrLanguage::Auto)
    }

    /// 提交并启动一个使用提交时语言预设的 OCR worker。
    ///
    /// `language` 是 Copy 枚举，在线程创建前捕获，运行中的 worker 不会读取
    /// 之后保存到运行时设置的值。
    pub fn start_with_language(
        &mut self,
        spec: JobSpec,
        image: CaptureImage,
        language: OcrLanguage,
    ) -> Result<JobTicket, PinoraError> {
        if spec.kind != JobKind::Ocr {
            return Err(PinoraError::new(
                ErrorCode::CommandRejected,
                "ocr job service accepts only JobKind::Ocr",
            ));
        }
        let ticket = self.supervisor.submit(spec)?;
        let cancellation = ticket.cancellation();
        let sender = self.sender.clone();
        let runner = self.runner.clone();
        let reference = JobResultRef::new(ticket.id, spec.asset);
        let worker = thread::Builder::new()
            .name(format!("pinora-ocr-{}", ticket.id.raw()))
            .spawn(move || {
                let result = runner.recognize_with_language(&image, language, &cancellation);
                let _ = sender.send(WorkerResult { reference, result });
            });
        let worker = worker.map_err(|error| {
            let _ = self.supervisor.fail(ticket.id);
            PinoraError::new(ErrorCode::Internal, format!("spawn ocr worker: {error}"))
        })?;
        self.workers.push(worker);
        Ok(ticket)
    }

    pub fn state(&self, id: JobId) -> Option<JobState> {
        self.supervisor.state(id)
    }

    /// 关闭 owner 后，所有属于该 owner 的 OCR worker 都会收到取消令牌。
    pub fn close_owner(&mut self, owner: JobOwner) -> usize {
        self.supervisor.close_owner(owner)
    }

    /// 应用退出时取消全部运行中的 OCR worker。
    pub fn cancel_all(&mut self) -> usize {
        self.supervisor.cancel_all()
    }

    pub(crate) fn cancel_all_and_wait(&mut self, timeout: Duration) -> WorkerWaitOutcome {
        let cancelled = self.cancel_all();
        let mut outcome = wait_for_workers(&mut self.workers, timeout);
        outcome.cancelled = cancelled;
        outcome
    }

    /// 轮询 worker 结果并依据当前 owner 的资产版本决定是否交付。
    pub fn poll<F>(&mut self, now_ms: u64, mut current_asset: F) -> Vec<OcrJobCompletion>
    where
        F: FnMut(JobOwner) -> Option<AssetRef>,
    {
        self.supervisor.expire_at(now_ms);
        let mut completions = Vec::new();
        while let Ok(worker) = self.receiver.try_recv() {
            let Some(spec) = self.supervisor.spec(worker.reference.job_id) else {
                continue;
            };
            match worker.result {
                Ok(result) => {
                    let Some(asset) = current_asset(spec.owner) else {
                        self.supervisor.close_owner(spec.owner);
                        completions.push(OcrJobCompletion::Discarded {
                            job_id: worker.reference.job_id,
                            terminal: JobTerminalState::OwnerClosed,
                        });
                        continue;
                    };
                    match self
                        .supervisor
                        .accept_result(worker.reference, asset, now_ms)
                    {
                        Ok(JobResultDisposition::Accepted(job)) => {
                            completions.push(OcrJobCompletion::Completed { job, result });
                        }
                        Ok(JobResultDisposition::Rejected(terminal)) => {
                            completions.push(OcrJobCompletion::Discarded {
                                job_id: worker.reference.job_id,
                                terminal,
                            });
                        }
                        Err(error) => {
                            completions.push(OcrJobCompletion::Failed {
                                job_id: worker.reference.job_id,
                                owner: spec.owner,
                                error,
                            });
                        }
                    }
                }
                Err(error) => {
                    let Some(asset) = current_asset(spec.owner) else {
                        self.supervisor.close_owner(spec.owner);
                        completions.push(OcrJobCompletion::Discarded {
                            job_id: worker.reference.job_id,
                            terminal: JobTerminalState::OwnerClosed,
                        });
                        continue;
                    };
                    if asset != worker.reference.asset {
                        let _ = self.supervisor.fail(worker.reference.job_id);
                        completions.push(OcrJobCompletion::Discarded {
                            job_id: worker.reference.job_id,
                            terminal: JobTerminalState::StaleAsset,
                        });
                        continue;
                    }
                    match self.supervisor.fail(worker.reference.job_id) {
                        Ok(JobState::Finished(JobTerminalState::Failed)) => {
                            completions.push(OcrJobCompletion::Failed {
                                job_id: worker.reference.job_id,
                                owner: spec.owner,
                                error,
                            });
                        }
                        Ok(JobState::Finished(terminal)) => {
                            completions.push(OcrJobCompletion::Discarded {
                                job_id: worker.reference.job_id,
                                terminal,
                            });
                        }
                        Ok(JobState::Running) => {}
                        Err(supervisor_error) => {
                            completions.push(OcrJobCompletion::Failed {
                                job_id: worker.reference.job_id,
                                owner: spec.owner,
                                error: supervisor_error,
                            });
                        }
                    }
                }
            }
        }
        let _ = reap_finished_workers(&mut self.workers);
        completions
    }
}

impl<R> Drop for OcrJobService<R>
where
    R: OcrRunner,
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
        AssetGeneration, CaptureMetadata, CorrelationId, DisplayId, ImageId, JobId, PixelRect,
        PixelSize, RgbaBuffer, SessionId,
    };

    #[derive(Debug)]
    struct SuccessRunner;

    impl OcrRunner for SuccessRunner {
        fn recognize(
            &self,
            _image: &CaptureImage,
            _cancellation: &JobCancellation,
        ) -> Result<OcrResult, PinoraError> {
            Ok(OcrResult::from_lines(
                Vec::new(),
                vec!["test".into()],
                "fake",
            ))
        }
    }

    #[derive(Debug)]
    struct FailureRunner;

    impl OcrRunner for FailureRunner {
        fn recognize(
            &self,
            _image: &CaptureImage,
            _cancellation: &JobCancellation,
        ) -> Result<OcrResult, PinoraError> {
            Err(PinoraError::new(ErrorCode::Internal, "fake ocr failed"))
        }
    }

    #[derive(Debug)]
    struct WaitForCancellationRunner;

    impl OcrRunner for WaitForCancellationRunner {
        fn recognize(
            &self,
            _image: &CaptureImage,
            cancellation: &JobCancellation,
        ) -> Result<OcrResult, PinoraError> {
            while !cancellation.is_cancelled() {
                thread::yield_now();
            }
            Err(PinoraError::new(ErrorCode::Cancelled, "fake ocr cancelled"))
        }
    }

    #[derive(Debug)]
    struct LanguageRecordingRunner {
        received: Arc<Mutex<Vec<OcrLanguage>>>,
    }

    impl OcrRunner for LanguageRecordingRunner {
        fn recognize(
            &self,
            _image: &CaptureImage,
            _cancellation: &JobCancellation,
        ) -> Result<OcrResult, PinoraError> {
            Err(PinoraError::new(
                ErrorCode::Internal,
                "language-aware runner must receive a frozen language",
            ))
        }

        fn recognize_with_language(
            &self,
            _image: &CaptureImage,
            language: OcrLanguage,
            _cancellation: &JobCancellation,
        ) -> Result<OcrResult, PinoraError> {
            self.received
                .lock()
                .expect("language recording mutex")
                .push(language);
            Ok(OcrResult::from_lines(
                Vec::new(),
                Vec::new(),
                "language-fake",
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

    fn spec(id: u64, asset: AssetRef, deadline_at_ms: u64) -> JobSpec {
        JobSpec::new(
            JobId::from_raw(id),
            CorrelationId::from_raw(id),
            asset,
            JobOwner::Session(SessionId::from_raw(id)),
            JobKind::Ocr,
            deadline_at_ms,
        )
    }

    fn poll_until<R, F>(
        service: &mut OcrJobService<R>,
        now_ms: u64,
        current_asset: F,
    ) -> Vec<OcrJobCompletion>
    where
        R: OcrRunner,
        F: FnMut(JobOwner) -> Option<AssetRef> + Copy,
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
    fn matching_owner_and_asset_deliver_result() {
        let image = sample_image(1);
        let asset = AssetRef::initial(image.id);
        let mut service = OcrJobService::with_runner(SuccessRunner);
        let ticket = service.start(spec(1, asset, 100), image).expect("start");
        let completions = poll_until(&mut service, 1, |_| Some(asset));

        assert!(matches!(
            completions.as_slice(),
            [OcrJobCompletion::Completed { job, result }]
                if job.id == ticket.id && job.asset == asset && result.engine == "fake"
        ));
        assert_eq!(
            service.state(ticket.id),
            Some(JobState::Finished(JobTerminalState::Completed))
        );
    }

    #[test]
    fn start_with_language_freezes_language_for_the_worker() {
        let image = sample_image(9);
        let asset = AssetRef::initial(image.id);
        let received = Arc::new(Mutex::new(Vec::new()));
        let runner = LanguageRecordingRunner {
            received: received.clone(),
        };
        let mut service = OcrJobService::with_runner(runner);

        let ticket = service
            .start_with_language(spec(9, asset, 100), image, OcrLanguage::English)
            .expect("start language-aware job");
        let completions = poll_until(&mut service, 1, |_| Some(asset));

        assert!(matches!(
            completions.as_slice(),
            [OcrJobCompletion::Completed { job, result }]
                if job.id == ticket.id && result.engine == "language-fake"
        ));
        assert_eq!(
            received
                .lock()
                .expect("language recording mutex")
                .as_slice(),
            [OcrLanguage::English]
        );
    }

    #[test]
    fn runner_failure_marks_only_running_job_as_failed() {
        let image = sample_image(2);
        let asset = AssetRef::initial(image.id);
        let mut service = OcrJobService::with_runner(FailureRunner);
        let ticket = service.start(spec(2, asset, 100), image).expect("start");
        let completions = poll_until(&mut service, 1, |_| Some(asset));

        assert!(matches!(
            completions.as_slice(),
            [OcrJobCompletion::Failed { job_id, error, .. }]
                if *job_id == ticket.id && error.code == ErrorCode::Internal
        ));
        assert_eq!(
            service.state(ticket.id),
            Some(JobState::Finished(JobTerminalState::Failed))
        );
    }

    #[test]
    fn closed_owner_discards_late_result() {
        let image = sample_image(3);
        let asset = AssetRef::initial(image.id);
        let job = spec(3, asset, 100);
        let owner = job.owner;
        let mut service = OcrJobService::with_runner(SuccessRunner);
        let ticket = service.start(job, image).expect("start");
        assert_eq!(service.close_owner(owner), 1);
        let completions = poll_until(&mut service, 1, |_| Some(asset));

        assert!(matches!(
            completions.as_slice(),
            [OcrJobCompletion::Discarded { job_id, terminal }]
                if *job_id == ticket.id && *terminal == JobTerminalState::OwnerClosed
        ));
    }

    #[test]
    fn expired_job_discards_worker_result() {
        let image = sample_image(4);
        let asset = AssetRef::initial(image.id);
        let mut service = OcrJobService::with_runner(WaitForCancellationRunner);
        let ticket = service.start(spec(4, asset, 10), image).expect("start");
        let completions = poll_until(&mut service, 10, |_| Some(asset));

        assert!(matches!(
            completions.as_slice(),
            [OcrJobCompletion::Discarded { job_id, terminal }]
                if *job_id == ticket.id && *terminal == JobTerminalState::TimedOut
        ));
    }

    #[test]
    fn cancellation_waits_for_worker_convergence() {
        let image = sample_image(8);
        let asset = AssetRef::initial(image.id);
        let mut service = OcrJobService::with_runner(WaitForCancellationRunner);
        let ticket = service.start(spec(8, asset, 100), image).expect("start");

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
    fn changed_asset_generation_discards_result() {
        let image = sample_image(5);
        let submitted_asset = AssetRef::initial(image.id);
        let current_asset = submitted_asset.advance().expect("advance generation");
        let mut service = OcrJobService::with_runner(SuccessRunner);
        let ticket = service
            .start(spec(5, submitted_asset, 100), image)
            .expect("start");
        let completions = poll_until(&mut service, 1, |_| Some(current_asset));

        assert!(matches!(
            completions.as_slice(),
            [OcrJobCompletion::Discarded { job_id, terminal }]
                if *job_id == ticket.id && *terminal == JobTerminalState::StaleAsset
        ));
    }

    #[test]
    fn changed_asset_generation_discards_failure() {
        let image = sample_image(6);
        let submitted_asset = AssetRef::initial(image.id);
        let current_asset = submitted_asset.advance().expect("advance generation");
        let mut service = OcrJobService::with_runner(FailureRunner);
        let ticket = service
            .start(spec(6, submitted_asset, 100), image)
            .expect("start");
        let completions = poll_until(&mut service, 1, |_| Some(current_asset));

        assert!(matches!(
            completions.as_slice(),
            [OcrJobCompletion::Discarded { job_id, terminal }]
                if *job_id == ticket.id && *terminal == JobTerminalState::StaleAsset
        ));
    }

    #[test]
    fn missing_owner_discards_failure() {
        let image = sample_image(7);
        let asset = AssetRef::initial(image.id);
        let mut service = OcrJobService::with_runner(FailureRunner);
        let ticket = service.start(spec(7, asset, 100), image).expect("start");
        let completions = poll_until(&mut service, 1, |_| None);

        assert!(matches!(
            completions.as_slice(),
            [OcrJobCompletion::Discarded { job_id, terminal }]
                if *job_id == ticket.id && *terminal == JobTerminalState::OwnerClosed
        ));
    }

    #[test]
    fn non_ocr_job_is_rejected_before_worker_starts() {
        let image = sample_image(6);
        let asset = AssetRef::new(image.id, AssetGeneration::from_raw(2).expect("generation"));
        let mut service = OcrJobService::with_runner(SuccessRunner);
        let rejected = JobSpec::new(
            JobId::from_raw(6),
            CorrelationId::from_raw(6),
            asset,
            JobOwner::Session(SessionId::from_raw(6)),
            JobKind::Export,
            100,
        );

        assert_eq!(
            service.start(rejected, image).unwrap_err().code,
            ErrorCode::CommandRejected
        );
    }
}
