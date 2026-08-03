//! 应用服务自有 worker 的有界回收工具。

use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// 退出等待的实际结果。`unfinished` 表示取消期限后仍未结束的协作式 worker。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkerWaitOutcome {
    pub cancelled: usize,
    pub joined: usize,
    pub panicked: usize,
    pub unfinished: usize,
}

impl WorkerWaitOutcome {
    fn merge(&mut self, other: Self) {
        self.cancelled += other.cancelled;
        self.joined += other.joined;
        self.panicked += other.panicked;
        self.unfinished = other.unfinished;
    }
}

/// 只 join 已完成 worker，绝不在正常轮询路径阻塞。
pub fn reap_finished_workers(workers: &mut Vec<JoinHandle<()>>) -> WorkerWaitOutcome {
    let mut pending = Vec::with_capacity(workers.len());
    let mut outcome = WorkerWaitOutcome::default();
    for worker in workers.drain(..) {
        if worker.is_finished() {
            outcome.joined += 1;
            if worker.join().is_err() {
                outcome.panicked += 1;
            }
        } else {
            pending.push(worker);
        }
    }
    outcome.unfinished = pending.len();
    *workers = pending;
    outcome
}

/// 在固定期限内重复回收已结束 worker。超过期限后返回残留数量而不无限等待。
pub fn wait_for_workers(workers: &mut Vec<JoinHandle<()>>, timeout: Duration) -> WorkerWaitOutcome {
    let deadline = Instant::now() + timeout;
    let mut outcome = WorkerWaitOutcome::default();
    loop {
        let reaped = reap_finished_workers(workers);
        outcome.merge(reaped);
        if outcome.unfinished == 0 || Instant::now() >= deadline {
            return outcome;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_finished_worker_without_waiting_for_pending_one() {
        let finished = std::thread::spawn(|| {});
        let pending = std::thread::spawn(|| std::thread::sleep(Duration::from_millis(20)));
        let mut workers = vec![finished, pending];

        std::thread::sleep(Duration::from_millis(2));
        let first = reap_finished_workers(&mut workers);
        assert_eq!(first.joined, 1);
        assert_eq!(first.unfinished, 1);

        let final_outcome = wait_for_workers(&mut workers, Duration::from_secs(1));
        assert_eq!(final_outcome.joined, 1);
        assert_eq!(final_outcome.unfinished, 0);
    }
}
