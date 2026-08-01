# 任务 023：建立受监督 OCR 应用服务

- 状态：进行中
- 计划：`.context/plans/023_ocr_job_service.md`
- 规模：中
- 依赖：`.context/tasks/019_asset_generation_contract.md`、`.context/tasks/021_job_supervision_contract.md`、`.context/tasks/022_ocr_process_lifecycle.md`
- 生产行为变更：无；新增尚未接入旧 UI 的 OCR 服务与状态机。

## 目的

把 OCR 异步执行、取消令牌和结果提交从窗口事件处理解耦，使贴图/Overlay 接入时可以基于 owner 与 generation 安全接收或丢弃结果。

## 任务目标

新增 `OcrJobService` 和可注入 `OcrRunner`：服务提交只读 `JobSpec`，worker 使用 `JobCancellation` 执行 OCR 并回传 `JobResultRef`；服务轮询时只有当前 owner 仍提供匹配 `AssetRef` 的结果才交付。失败、取消、owner 关闭、超时和陈旧结果均不得更新调用方状态。

## 影响路径

- `crates/pinora-app/src/job_supervisor.rs` 的失败终态或只读任务元数据查询。
- 新增 `crates/pinora-app/src/ocr_job.rs`、公共导出与离线测试。
- 必要的 `pinora-core` 任务终态定义与测试。
- 当前计划、任务、系统概览和风险登记。

## 兼容性

- 接口：只新增服务/runner 和监督器查询，不删除 OCR 同步 API 或改动已有窗口接口。
- 数据/状态：不改 OCR 内容、截图、持久化、稳定状态字符串、租户或权限语义。
- 外部副作用：测试只启动 Rust worker thread 和 fake runner；不调用 Tesseract、桌面、网络或共享服务。

## 回滚点

删除新增服务、补充的监督器 API 和测试即可；旧 `desktop_shell` OCR 行为未调用此模块。

## 验证场景

- 匹配 owner/资产的 OCR 成功结果被交付，任务完成。
- runner 错误仅把运行任务变为失败；已取消任务的错误结果被丢弃。
- owner 关闭、截止时间到达和当前 generation 变化后，晚到结果被丢弃。
- fake runner 可验证全部场景，无需 Tesseract 或 GUI。

## 范围

- 补充受监督 OCR 所需的最小任务状态机能力。
- 新增 OCR 应用服务、生产 runner 和 fake runner 契约测试。
- 更新必要公共导出与上下文事实。

## 非目标

- 不改 `desktop_shell` 的同步贴图 OCR 或 Overlay OCR 线程。
- 不实现 UI 进度、并发限制、队列持久化、缓存、OCR 文本选择或剪贴板迁移。
- 不把服务单测误称为桌面闭环或真实 OCR 质量验证。

## 预期文件

- `crates/pinora-app/src/job_supervisor.rs`、新增 `ocr_job.rs`、`crates/pinora-app/src/lib.rs`。
- 视实现需要的 `crates/pinora-core/src/job.rs` 及其导出。
- 当前计划、任务、`.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- OCR worker 结果经 `JobSupervisor` 的 owner/generation/终态校验后才可交付。
- fake runner 测试覆盖成功、失败、owner 关闭、超时和陈旧 generation。
- 不新增 UI 依赖、警告抑制或真实外部服务调用；所有约定质量门禁通过。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app job_supervisor::tests -- --nocapture`
- `cargo test -p pinora-app ocr_job::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：worker 结果轮询的测试可能产生线程时序不稳定。缓解：fake runner 立即返回或只等待取消，并以有界轮询收集结果。
- 风险：新增失败终态会被误当作用户可见业务状态。缓解：它只存在于进程内任务监督器，不写入 `AppState` 或持久化。
- 回滚：移除本任务新增模块/导出/状态机方法；现有 UI 不受影响。

## 完成记录

- 初始证据：022 已使 OCR 适配器可取消，但 `desktop_shell.rs::run_pin_ocr` 仍在 UI 线程直接调用同步 `recognize_image`，Overlay OCR 仍自行 `thread::spawn`；两条路径都不构造 `JobSpec` 或校验 `AssetRef`。
- 状态：已完成（2026-08-01）。
- 实际变更：`JobSupervisor` 增加 `Failed` 终态、任务 `spec` 查询和 `cancel_all`；新增 `OcrRunner`、`LocalOcrRunner`、`OcrJobService` 与 `OcrJobCompletion`。服务提交带 `JobSpec` 的 OCR，worker 回送 `JobResultRef`，轮询时按当前 owner 资产校验并交付、失败或丢弃结果。
- 验证：`job_supervisor::tests` 6/6、`ocr_job::tests` 6/6；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（60 app + 39 core 通过，2 个真实桌面测试忽略）、`git diff --check` 与上下文校验均通过。
- 残留风险：`desktop_shell` 仍未接入服务，现有 UI OCR 行为没有改变；服务本身不提供线程 join，退出等待与窗口 owner 绑定留待下一任务。
