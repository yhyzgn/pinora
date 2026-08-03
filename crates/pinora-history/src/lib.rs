//! Pinora 历史工作流边界。
//!
//! 本 crate 负责受管历史文件的索引、清理策略和异步图像读取；窗口资源、Panel
//! 交互和唯一 EventLoop 仍由 `pinora-app` 持有。

mod history_export;
mod history_load_job;
mod history_session;
mod retention;

pub use history_export::{
    HistoryCleanup, HistoryExportCandidate, HistoryPolicyReconcile, clear_history_entries,
    delete_history_entry, history_candidate_for_export, load_history_index,
    reconcile_history_policy, record_history_candidate,
};
pub use history_load_job::{
    HistoryLoadCompletion, HistoryLoadInput, HistoryLoadJobService, HistoryLoadPayload,
    HistoryLoadPreparation, HistoryLoadRunner, LocalHistoryLoadRunner,
};
pub use history_session::{ActiveHistoryLoad, HistoryLoadIntent, HistoryLoadRequest};
pub use retention::history_retention_cutoff_ms;
