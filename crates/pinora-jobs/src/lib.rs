//! Pinora 通用任务生命周期底座。
//!
//! 监督器只管理任务身份、取消和结果门禁；具体服务继续拥有自己创建的线程或子进程，
//! 并在本 crate 提供的有界回收工具中完成退出。

mod job_supervisor;
mod worker_lifecycle;

pub use job_supervisor::{
    AcceptedJobResult, JobCancellation, JobResultDisposition, JobState, JobSupervisor, JobTicket,
};
pub use worker_lifecycle::{WorkerWaitOutcome, reap_finished_workers, wait_for_workers};
