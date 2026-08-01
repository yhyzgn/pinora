# 计划 029：后台 worker 退出收敛

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/029_worker_shutdown_convergence.md`

## 目标

使 `OcrJobService` 与 `ExportJobService` 保留自己创建的 worker 线程句柄，应用退出时先取消所有任务、在有界期限内回收已完成 worker，并明确报告仍未收敛的数量，避免把单纯取消请求视为完成回收。

## 非目标

- 不引入异步运行时、线程池、强制终止 Rust 线程或全局线程扫描。
- 不实现复杂子孙进程组回收、进程级 watchdog 或跨平台服务管理。
- 不改变 OCR/导出输入、领域事件或 UI 产品功能。

## 约束

- `JobSupervisor` 继续只管理任务状态，不持有线程句柄；worker 句柄属于具体应用服务。
- 只 join 已完成的 `JoinHandle`；等待必须有截止时间，超时后明确返回未收敛数量，不能无限阻塞退出。
- worker 取消仍为协作式，外部 child 的回收责任保持在 022/025 适配器。
- 测试使用可取消 fake runner，不启动真实 OCR、系统剪贴板或桌面。

## 依赖关系

- 依赖 023/024 的 OCR 服务与桌面取消路径。
- 依赖 026/027 的导出服务与桌面取消路径。

## 阶段

1. 增加共享的已完成 worker 回收和有界等待工具。
2. 两个服务保存 worker 句柄，轮询期间回收已结束线程，提供取消并等待 API。
3. 桌面退出调用有界收敛并记录未完成 worker；fake runner 测试锁定行为。

## 检查点

- 服务启动的每个 worker 都有可回收句柄，不再 fire-and-forget。
- 退出最多等待固定期限；timeout 不阻塞主线程无限期，但报告 residual worker。
- worker cancel、任务终态和结果门禁保持既有语义。

## 计划级风险

- 协作式 Rust worker 可能忽略取消，不能由本计划强杀；此时只能报告未收敛，不能伪造已回收。
- `JoinHandle::is_finished` 和 join 仅证明线程结束，不证明复杂外部后端的孙进程已退出，后者继续作为平台风险。

## 完成标准

- OCR/导出服务均持有并有界回收 worker，桌面退出使用该 API。
- fake runner 覆盖取消后收敛；严格门禁通过。
- 未收敛 worker、GUI E2E 和子孙进程组缺口明确记录。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：新增 `worker_lifecycle`，只 join `is_finished` 的 worker 并在固定期限内轮询回收。OCR/导出服务保存每个 `JoinHandle`，正常 poll 回收已结束 worker，`cancel_all_and_wait` 返回取消、join、panic 和未收敛计数；桌面退出以 2 秒上限调用并输出实际计数。Drop 仍以短时有界等待作为兜底。
- 验证：OCR 服务 7/7、导出服务 7/7、共享 worker 工具 1/1；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（app 74 项通过、2 个真实桌面测试忽略；core 39 项通过）、差异检查和上下文校验通过。
- 残留风险：忽略取消的 Rust worker 只能在期限后报告未收敛，不能安全强杀；join 不证明外部后端孙进程组已退出；真实退出探针和 GUI E2E 未运行。
