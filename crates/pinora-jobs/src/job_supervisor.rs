//! 应用层任务监督器：管理取消、owner 关闭、超时和结果提交门禁。
//!
//! 该模块只管理内存状态与协作式取消令牌。具体 OCR/导出适配器必须负责停止并
//! 回收自己创建的子进程或线程，且只能在本模块接受结果后更新应用状态。

use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use pinora_core::{
    AssetRef, ErrorCode, JobId, JobOwner, JobResultRef, JobSpec, JobTerminalState, PinoraError,
};

/// 供工作单元轮询的协作式取消令牌。
#[derive(Debug, Clone)]
pub struct JobCancellation(Arc<AtomicBool>);

impl JobCancellation {
    /// 创建不受监督器登记的令牌，供保留兼容性的同步适配器使用。
    pub fn standalone() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// 工作单元应在可中断点检查此标志并尽快退出。
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}

/// 将任务交给工作单元时提供的身份与取消令牌。
#[derive(Debug, Clone)]
pub struct JobTicket {
    pub id: JobId,
    cancellation: JobCancellation,
}

impl JobTicket {
    pub fn cancellation(&self) -> JobCancellation {
        self.cancellation.clone()
    }
}

/// 任务当前状态。终态一经进入不可逆转。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Running,
    Finished(JobTerminalState),
}

/// 结果被应用层接受后可安全使用的冻结元数据。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedJobResult {
    pub id: JobId,
    pub owner: JobOwner,
    pub asset: AssetRef,
}

/// 结果提交后的判定。拒绝也会将任务置于不可接受结果的终态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobResultDisposition {
    Accepted(AcceptedJobResult),
    Rejected(JobTerminalState),
}

#[derive(Debug, Clone)]
struct TrackedJob {
    spec: JobSpec,
    state: JobState,
    cancellation: JobCancellation,
}

/// 进程内任务状态机。
///
/// 该监督器不运行工作单元。调用者向其注册任务并拿到 `JobTicket`，由具体适配器
/// 使用令牌完成协作式取消；结果回到应用事件循环后必须先调用 `accept_result`。
#[derive(Debug, Default)]
pub struct JobSupervisor {
    jobs: HashMap<JobId, TrackedJob>,
    closed_owners: HashSet<JobOwner>,
}

impl JobSupervisor {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个尚未开始的受监督任务。
    pub fn submit(&mut self, spec: JobSpec) -> Result<JobTicket, PinoraError> {
        if self.closed_owners.contains(&spec.owner) {
            return Err(PinoraError::new(
                ErrorCode::InvalidState,
                format!("job owner is closed: {:?}", spec.owner),
            ));
        }
        if self.jobs.contains_key(&spec.id) {
            return Err(PinoraError::new(
                ErrorCode::InvalidState,
                format!("job already registered: {}", spec.id),
            ));
        }

        let cancellation = JobCancellation::standalone();
        self.jobs.insert(
            spec.id,
            TrackedJob {
                spec,
                state: JobState::Running,
                cancellation: cancellation.clone(),
            },
        );
        Ok(JobTicket {
            id: spec.id,
            cancellation,
        })
    }

    pub fn state(&self, id: JobId) -> Option<JobState> {
        self.jobs.get(&id).map(|job| job.state)
    }

    /// 返回任务冻结的元数据；调用方不得据此绕过结果提交门禁。
    pub fn spec(&self, id: JobId) -> Option<JobSpec> {
        self.jobs.get(&id).map(|job| job.spec)
    }

    /// 取消单个仍在运行的任务；重复取消返回其既有终态。
    pub fn cancel(&mut self, id: JobId) -> Result<JobState, PinoraError> {
        let job = self.job_mut(id)?;
        if job.state == JobState::Running {
            job.cancellation.cancel();
            job.state = JobState::Finished(JobTerminalState::Cancelled);
        }
        Ok(job.state)
    }

    /// 记录工作单元失败；已经取消、超时、关闭或完成的任务保持既有终态。
    pub fn fail(&mut self, id: JobId) -> Result<JobState, PinoraError> {
        let job = self.job_mut(id)?;
        if job.state == JobState::Running {
            job.state = JobState::Finished(JobTerminalState::Failed);
        }
        Ok(job.state)
    }

    /// 使一个领域 owner 失效，并取消其所有尚在运行的任务。
    pub fn close_owner(&mut self, owner: JobOwner) -> usize {
        self.closed_owners.insert(owner);
        let mut cancelled = 0;
        for job in self.jobs.values_mut() {
            if job.spec.owner == owner && job.state == JobState::Running {
                job.cancellation.cancel();
                job.state = JobState::Finished(JobTerminalState::OwnerClosed);
                cancelled += 1;
            }
        }
        cancelled
    }

    /// 取消所有仍在运行的任务，供应用退出编排使用。
    pub fn cancel_all(&mut self) -> usize {
        let mut cancelled = 0;
        for job in self.jobs.values_mut() {
            if job.state == JobState::Running {
                job.cancellation.cancel();
                job.state = JobState::Finished(JobTerminalState::Cancelled);
                cancelled += 1;
            }
        }
        cancelled
    }

    /// 将截至 `now_ms` 已到期的运行任务置为超时，并请求协作式取消。
    pub fn expire_at(&mut self, now_ms: u64) -> Vec<JobId> {
        let mut timed_out = Vec::new();
        for (id, job) in &mut self.jobs {
            if job.state == JobState::Running && job.spec.is_expired_at(now_ms) {
                job.cancellation.cancel();
                job.state = JobState::Finished(JobTerminalState::TimedOut);
                timed_out.push(*id);
            }
        }
        timed_out
    }

    /// 判断任务结果是否仍可提交。
    ///
    /// `result.asset` 必须等于提交时的资产，且必须仍等于调用方传入的当前资产。
    /// 任一不匹配均表示该结果已陈旧，不能更新 UI 或领域状态。
    pub fn accept_result(
        &mut self,
        result: JobResultRef,
        current_asset: AssetRef,
        now_ms: u64,
    ) -> Result<JobResultDisposition, PinoraError> {
        self.expire_at(now_ms);
        let job = self.job_mut(result.job_id)?;
        if let JobState::Finished(terminal) = job.state {
            return Ok(JobResultDisposition::Rejected(terminal));
        }
        if job.spec.asset != result.asset || !current_asset.accepts_result(result.asset) {
            job.cancellation.cancel();
            job.state = JobState::Finished(JobTerminalState::StaleAsset);
            return Ok(JobResultDisposition::Rejected(JobTerminalState::StaleAsset));
        }

        job.state = JobState::Finished(JobTerminalState::Completed);
        Ok(JobResultDisposition::Accepted(AcceptedJobResult {
            id: job.spec.id,
            owner: job.spec.owner,
            asset: job.spec.asset,
        }))
    }

    fn job_mut(&mut self, id: JobId) -> Result<&mut TrackedJob, PinoraError> {
        self.jobs
            .get_mut(&id)
            .ok_or_else(|| PinoraError::new(ErrorCode::NotFound, format!("job not found: {id}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinora_core::{AssetGeneration, CorrelationId, ImageId, JobKind, PinId, SessionId};

    fn asset(image: u64, generation: u64) -> AssetRef {
        AssetRef::new(
            ImageId::from_raw(image),
            AssetGeneration::from_raw(generation).expect("non-zero generation"),
        )
    }

    fn spec(id: u64, owner: JobOwner, asset: AssetRef, deadline_at_ms: u64) -> JobSpec {
        JobSpec::new(
            JobId::from_raw(id),
            CorrelationId::from_raw(id + 100),
            asset,
            owner,
            JobKind::Ocr,
            deadline_at_ms,
        )
    }

    #[test]
    fn accepts_matching_result_and_completes_job() {
        let asset = asset(1, 1);
        let job = spec(1, JobOwner::Pin(PinId::from_raw(9)), asset, 100);
        let mut supervisor = JobSupervisor::new();
        let ticket = supervisor.submit(job).expect("submit");

        let disposition = supervisor
            .accept_result(JobResultRef::new(ticket.id, asset), asset, 99)
            .expect("known job");

        assert_eq!(
            disposition,
            JobResultDisposition::Accepted(AcceptedJobResult {
                id: ticket.id,
                owner: job.owner,
                asset,
            })
        );
        assert_eq!(
            supervisor.state(ticket.id),
            Some(JobState::Finished(JobTerminalState::Completed))
        );
        assert!(!ticket.cancellation().is_cancelled());
    }

    #[test]
    fn cancelled_job_rejects_late_result_and_signals_worker() {
        let asset = asset(2, 1);
        let job = spec(2, JobOwner::Pin(PinId::from_raw(10)), asset, 100);
        let mut supervisor = JobSupervisor::new();
        let ticket = supervisor.submit(job).expect("submit");

        assert_eq!(
            supervisor.cancel(ticket.id).expect("known job"),
            JobState::Finished(JobTerminalState::Cancelled)
        );
        assert!(ticket.cancellation().is_cancelled());
        assert_eq!(
            supervisor
                .accept_result(JobResultRef::new(ticket.id, asset), asset, 1)
                .expect("known job"),
            JobResultDisposition::Rejected(JobTerminalState::Cancelled)
        );
    }

    #[test]
    fn worker_failure_only_transitions_a_running_job() {
        let asset = asset(8, 1);
        let job = spec(8, JobOwner::Pin(PinId::from_raw(8)), asset, 100);
        let mut supervisor = JobSupervisor::new();
        let ticket = supervisor.submit(job).expect("submit");

        assert_eq!(
            supervisor.fail(ticket.id).expect("known job"),
            JobState::Finished(JobTerminalState::Failed)
        );
        assert_eq!(
            supervisor.cancel(ticket.id).expect("known job"),
            JobState::Finished(JobTerminalState::Failed)
        );
        assert_eq!(supervisor.spec(ticket.id), Some(job));
    }

    #[test]
    fn closing_owner_cancels_only_its_running_jobs() {
        let left_asset = asset(3, 1);
        let right_asset = asset(4, 1);
        let left_owner = JobOwner::Session(SessionId::from_raw(4));
        let right_owner = JobOwner::Pin(PinId::from_raw(5));
        let mut supervisor = JobSupervisor::new();
        let left = supervisor
            .submit(spec(3, left_owner, left_asset, 100))
            .expect("left submit");
        let right = supervisor
            .submit(spec(4, right_owner, right_asset, 100))
            .expect("right submit");

        assert_eq!(supervisor.close_owner(left_owner), 1);
        assert!(left.cancellation().is_cancelled());
        assert!(!right.cancellation().is_cancelled());
        assert_eq!(
            supervisor.state(left.id),
            Some(JobState::Finished(JobTerminalState::OwnerClosed))
        );
        assert_eq!(supervisor.state(right.id), Some(JobState::Running));
        assert_eq!(
            supervisor
                .submit(spec(5, left_owner, asset(5, 1), 100))
                .unwrap_err()
                .code,
            ErrorCode::InvalidState
        );
    }

    #[test]
    fn timeout_rejects_result_at_deadline() {
        let asset = asset(6, 1);
        let job = spec(6, JobOwner::Pin(PinId::from_raw(6)), asset, 50);
        let mut supervisor = JobSupervisor::new();
        let ticket = supervisor.submit(job).expect("submit");

        assert_eq!(supervisor.expire_at(50), vec![ticket.id]);
        assert!(ticket.cancellation().is_cancelled());
        assert_eq!(
            supervisor
                .accept_result(JobResultRef::new(ticket.id, asset), asset, 50)
                .expect("known job"),
            JobResultDisposition::Rejected(JobTerminalState::TimedOut)
        );
    }

    #[test]
    fn stale_asset_generation_is_rejected() {
        let captured_asset = asset(7, 1);
        let current_asset = captured_asset.advance().expect("generation advances");
        let job = spec(7, JobOwner::Pin(PinId::from_raw(7)), captured_asset, 100);
        let mut supervisor = JobSupervisor::new();
        let ticket = supervisor.submit(job).expect("submit");

        assert_eq!(
            supervisor
                .accept_result(
                    JobResultRef::new(ticket.id, captured_asset),
                    current_asset,
                    1,
                )
                .expect("known job"),
            JobResultDisposition::Rejected(JobTerminalState::StaleAsset)
        );
        assert!(ticket.cancellation().is_cancelled());
        assert_eq!(
            supervisor.state(ticket.id),
            Some(JobState::Finished(JobTerminalState::StaleAsset))
        );
    }
}
