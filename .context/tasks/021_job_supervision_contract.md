# 任务 021：建立任务监督基础契约

- 状态：进行中
- 计划：`.context/plans/021_job_supervision_contract.md`
- 规模：中
- 依赖：`.context/tasks/019_asset_generation_contract.md`、`.context/tasks/020_capture_capability_truth.md`
- 生产行为变更：无；新增尚未接入旧 UI 的应用/领域契约。

## 目的

为 OCR、导出、剪贴板和捕获等耗时操作建立统一且可验证的任务归属、取消、截止时间和陈旧结果拒绝语义，消除后续迁移只能依赖窗口指针或无归属线程的前置障碍。

## 任务目标

新增稳定 `JobId`、`JobOwner` 和 `JobKind` 领域值对象，以及只管理内存状态的 `JobSupervisor`。它必须在任务仍运行、owner 有效、资产 `AssetRef` 完全匹配且未超过截止时间时接受结果；取消、关闭 owner、超时或 generation 不匹配后拒绝结果。

## 影响路径

- `crates/pinora-core/src/ids.rs`、新增聚焦的 job 领域模块及公共导出。
- `crates/pinora-app/src/` 下新增任务监督模块和离线单元测试。
- `crates/pinora-app/src/lib.rs` 公共导出。
- 当前计划、任务、系统概览与风险登记。

## 兼容性

- 接口：只新增类型和监督器 API，不修改既有 `Command`、`AppState` 或 OCR 公共函数。
- 数据/状态：不改持久化、稳定状态字符串、租户或权限语义；任务状态仅进程内。
- 生命周期：不启动线程、不访问真实桌面、不会连接外部服务。

## 外部副作用

无。仅执行离线 Rust 构建与单元测试，不启动图形会话、`tesseract` 或共享基础设施。

## 回滚点

反转新增的领域模块、监督器、导出和测试即可；现有截图、OCR、导出与窗口行为不受影响。

## 验证场景

- 注册任务后，相同 owner 与 `AssetRef` 的结果被接受并进入完成终态。
- 显式取消、owner 关闭或超时后，结果被拒绝并保留正确终态。
- 资产 ID 相同但 generation 变化时，旧结果被拒绝。
- 取消同一 owner 的全部任务不会影响其他 owner 的运行任务。

## 范围

- 新增最小纯领域任务标识/归属类型与内存监督器。
- 用确定性时钟或显式截止时间建立离线契约测试。
- 更新必要公共导出与上下文事实。

## 非目标

- 不接入或删除 `desktop_shell` 的 OCR 线程、外部进程、剪贴板线程或图形窗口。
- 不实现线程池、实际进度上报、持久队列、重试策略或跨进程任务恢复。
- 不声称已有子进程被取消或真实桌面 OCR 已验证。

## 预期文件

- `crates/pinora-core/src/ids.rs`、新增任务领域模块、`crates/pinora-core/src/lib.rs`。
- `crates/pinora-app/src/job_supervisor.rs`、`crates/pinora-app/src/lib.rs`。
- 当前计划、任务、`.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- 任务元数据完整表达 ID、关联 ID、资产版本、owner、类型和截止时间。
- 监督器拒绝取消、关闭、超时和陈旧 generation 的结果，且不依赖真实线程或 UI。
- 不新增 lint 抑制；所有约定质量门禁通过。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-core job::tests -- --nocapture`
- `cargo test -p pinora-app job_supervisor::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：纯内存监督器被错误理解为已具备强制回收外部进程的能力。缓解：类型名称、文档和完成记录明确其只定义提交门禁；后续适配器持有自己的进程句柄。
- 风险：将 `PinId` 与窗口句柄混为 owner。缓解：`JobOwner` 仅引用领域 ID 或会话 ID，不含 UI 类型。
- 回滚：删除本任务新增模块、导出和测试；既有运行时没有调用该模块。

## 完成记录

- 初始证据：`desktop_shell.rs` 对 Overlay OCR 直接 `thread::spawn`，对贴图 OCR 直接同步调用；`ocr.rs` 使用固定 30 秒等待和外部 `kill -9`，没有任务 owner、取消句柄或 `AssetRef` generation 校验。
- 状态：已完成（2026-08-01）。
- 实际变更：新增 `JobId`、`SessionId`、`JobOwner`、`JobKind`、`JobSpec`、`JobResultRef` 和 `JobTerminalState`；新增 `JobSupervisor` 管理内存中的任务状态、协作式取消令牌、owner 关闭、显式取消、超时以及当前资产 generation 的提交校验。
- 验证：`cargo test -p pinora-core job::tests -- --nocapture`（1/1）、`cargo test -p pinora-app job_supervisor::tests -- --nocapture`（5/5）、`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（48 app + 39 core 通过，2 个真实桌面测试忽略）、`git diff --check` 与上下文校验均通过。
- 残留风险：没有迁移 OCR/导出/剪贴板或 GUI 路径；`JobSupervisor` 不拥有子进程句柄，因此不是外部进程强制终止机制。后续 OCR 适配器必须把结果回送给监督器并实现自己的受控回收。
