# 任务 029：实现后台 worker 有界退出收敛

- 状态：已完成
- 计划：`.context/plans/029_worker_shutdown_convergence.md`
- 规模：中
- 依赖：`.context/tasks/024_desktop_ocr_integration.md`、`.context/tasks/027_desktop_export_job_integration.md`
- 生产行为变更：是；应用退出会在固定期限内等待已取消的 OCR/导出 worker 收敛，并把未完成数量作为诊断输出。

## 目的

修复 OCR/导出服务仅保存 channel、Drop 时只发取消信号而不保留 worker 句柄的问题，使退出路径能有证据地区分已 join 与仍在运行的协作式任务。

## 任务目标

新增共享 worker 回收工具，`OcrJobService`/`ExportJobService` 在启动时保存 `JoinHandle`，轮询时回收已完成线程，提供 `cancel_all_and_wait(timeout)` 返回已取消、已 join、panic 与未收敛计数。桌面壳退出使用该 API 并只报告事实。

## 影响路径

- 新增 `crates/pinora-app/src/worker_lifecycle.rs`。
- `crates/pinora-app/src/ocr_job.rs`、`export_job.rs`、`desktop_shell.rs`、`lib.rs`。
- 当前计划、任务、系统概览和风险登记。

## 兼容性

- 接口：仅新增服务 shutdown API；不改变 OCR/导出用户命令、领域状态或持久化。
- 生命周期：退出等待有界，worker 未收敛时保留取消状态并输出数量，不无限阻塞。
- 外部副作用：测试只用 Rust fake worker；生产仍由现有 child 适配器负责外部进程回收。

## 外部副作用

退出路径最多等待固定时间；不调用全局 kill、不访问网络或真实桌面。

## 回滚点

移除共享工具和服务句柄字段，恢复仅取消的退出路径；保留任务监督与 child 回收实现。

## 验证场景

- 可取消 fake OCR/导出 worker 接到 `cancel_all_and_wait` 后被 join，未收敛为零。
- 已完成 worker 在正常 poll 后被 join，不在服务内累积句柄。
- 超过截止时间的 worker 不无限阻塞 shutdown，返回非零未收敛数。
- desktop 退出日志分别报告取消、已 join 和残留数，不把残留称为成功回收。

## 范围

- 共享有界 join 工具与服务句柄管理。
- OCR/导出服务 shutdown API 和 fake runner 测试。
- desktop 退出调用与上下文更新。

## 非目标

- 不强制杀死 Rust 线程、孙进程组，不改 OCR/剪贴板协议或 UI 流程。
- 不引入 runtime/线程池/第三方依赖，不做 GUI E2E。

## 预期文件

- `crates/pinora-app/src/worker_lifecycle.rs`。
- `crates/pinora-app/src/ocr_job.rs`、`export_job.rs`、`desktop_shell.rs`、`lib.rs`。
- `.context/plans/029_worker_shutdown_convergence.md`、`.context/tasks/029_worker_shutdown_convergence.md`。
- `.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- 两个服务可有界等待其已取消 worker，并返回真实收敛计数。
- desktop 退出不再只报告“已取消”，而是报告 join/residual 证据。
- 无 lint 抑制，离线测试及完整质量门禁通过。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app ocr_job::tests -- --nocapture`
- `cargo test -p pinora-app export_job::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `rg -n 'JoinHandle|cancel_all_and_wait|unfinished' crates/pinora-app/src`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：坏 runner 永不响应取消。缓解：有界等待后报告未收敛，不阻塞退出也不伪造回收。
- 风险：worker panic 未被任务状态机捕捉。缓解：join 计数 panic，保持任务已知终态/诊断，不使用 warning suppression。
- 回滚：回退句柄管理与退出调用；保留所有 worker 结果门禁。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：`OcrJobService`/`ExportJobService` 用 `thread::Builder::spawn` 后丢弃 `JoinHandle`；`DesktopApp` 退出只调用 `cancel_all` 并打印取消数量，不能证明 worker 已结束。
- 实际变更：新增 `WorkerWaitOutcome`、已结束 worker 回收与有界等待工具；两个服务保留 `JoinHandle`，poll 回收完成线程，`cancel_all_and_wait` 返回取消/join/panic/残留计数。桌面退出改为 2 秒有界等待并输出四项真实计数，Drop 仅作 50ms 兜底等待。
- 验证：OCR 服务 7/7、导出服务 7/7、worker 生命周期工具 1/1；workspace check、严格 Clippy、workspace 测试（74 app 通过/2 忽略，39 core 通过）、差异检查和上下文校验通过。
- 未覆盖项：未对永久忽略取消的 worker 做真实退出探针；不会强杀 Rust 线程或外部孙进程组；GUI E2E 仍缺失。
