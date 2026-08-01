# 任务 022：重做 OCR 子进程生命周期

- 状态：进行中
- 计划：`.context/plans/022_ocr_process_lifecycle.md`
- 规模：中
- 依赖：`.context/tasks/021_job_supervision_contract.md`
- 生产行为变更：是；OCR 取消和超时改为受控回收自身 `Child`，错误码可区分取消与超时。

## 目的

消除 OCR 适配器的外部 `kill -9`、无归属等待线程和错误分支残留临时 PNG，使后续将 OCR 接入任务监督器时有可靠的进程边界。

## 任务目标

保留 `recognize_image` 的同步兼容入口，新增使用 `JobCancellation` 的 OCR 调用入口；将 `tesseract` 进程启动、轮询、取消、超时和 `wait` 收敛到同一模块。为取消、超时、输出上限和临时文件清理建立不依赖 `tesseract` 的本地进程测试。

## 影响路径

- `crates/pinora-app/src/ocr.rs` 及其单元测试。
- `crates/pinora-app/src/lib.rs` 的 OCR 公共导出。
- `crates/pinora-app/src/job_supervisor.rs` 的令牌构造可见性（如确有需要）。
- `crates/pinora-core/src/error.rs` 的稳定错误码及测试。
- 当前计划、任务、系统概览和风险登记。

## 兼容性

- 接口：保留既有 `recognize_image`；新增可取消 OCR API 和两个错误码，不删除已有类型。
- 数据/状态：不改 OCR 结果、图像、持久化、租户或权限语义。
- 生命周期：子进程只在本机启动；测试只运行受控本地 shell，不连接共享基础设施或桌面会话。

## 外部副作用

本地单元测试会启动短生命周期的受控 shell 子进程，以验证取消、超时和输出上限后的 `wait` 回收；既有本机 `tesseract` 冒烟测试在已安装引擎时会运行，但不访问网络、桌面或共享服务。

## 回滚点

反转 OCR 适配器、错误码和测试改动即可恢复旧同步行为；不修改 UI 触发、截图资产或领域状态。

## 验证场景

- 已取消令牌在启动前拒绝 OCR，不创建子进程。
- 运行中的受控 child 遇取消或截止时间时被适配器终止并等待，分别返回取消/超时错误码。
- 子进程标准输出超过上限时被回收并返回受控错误。
- 临时 PNG 离开作用域后被删除，包含错误路径。
- 既有 TSV 解析和本机可用时的 `tesseract` 冒烟测试保持通过。

## 范围

- 重构 OCR CLI 适配器的进程/临时文件生命周期。
- 添加必要稳定错误码、公共可取消入口和本地契约测试。
- 更新系统事实与风险。

## 非目标

- 不迁移 `desktop_shell` 中的 Overlay/贴图 OCR 调用、不变更其 UI 行为。
- 不创建 OCR 任务队列、缓存、进度 UI、模型下载或跨平台引擎。
- 不把本地 shell 测试当作真实 OCR、桌面授权或全生命周期集成验证。

## 预期文件

- `crates/pinora-app/src/ocr.rs`、`crates/pinora-app/src/lib.rs`。
- 视实现需要的 `crates/pinora-app/src/job_supervisor.rs`、`crates/pinora-core/src/error.rs`。
- 当前计划、任务、`.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- 不再使用外部 `kill` 或无归属 `wait_with_output` 线程控制 OCR。
- 取消、超时、输出超限均有稳定且可测试的失败语义，且 child 已被回收。
- 成功、失败、取消和超时均不遗留 OCR 临时 PNG。
- 所有约定质量门禁通过。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app ocr::tests -- --nocapture`
- `cargo test -p pinora-core error::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：轮询增加很小的等待粒度。缓解：轮询仅限 OCR worker，不在 UI 线程运行，并使用较短固定间隔；后续使用平台运行时后再评估。
- 风险：直接 child kill 不能覆盖引擎自行派生的子孙进程。缓解：当前只支持直接 `tesseract` child；进程组回收需在平台适配验证后单独引入。
- 回滚：反转本任务的 OCR/error/test 改动，不影响任务监督器、截图或 UI。

## 完成记录

- 初始证据：`ocr.rs::run_tesseract_tsv` 把 `wait_with_output` 放入无归属线程，30 秒后执行外部 `kill -9 <pid>`，而 `recognize_image` 在 `run_tesseract_tsv` 返回错误时不会删除刚创建的临时 PNG。
- 状态：已完成（2026-08-01）。
- 实际变更：保留同步 `recognize_image`，新增 `recognize_image_with_cancellation`；Tesseract OCR 和 `--list-langs` 探测均交给同一受控 child 等待器，取消、超时和 16 MiB 输出上限后使用适配器持有的 `Child::kill` 与 `wait` 回收。`TempPng` 在所有返回路径用 `Drop` 删除临时文件。
- 稳定错误：`ErrorCode` 新增 `Cancelled`、`TimedOut`、`ResourceLimitExceeded`；适配器不再执行外部 `kill`，也不再启动无归属 `wait_with_output` 线程。
- 验证：OCR 测试覆盖取消、超时、输出超限、临时文件清理、TSV 解析和本机可用时的 Tesseract 冒烟，共 8/8 通过；错误码 1/1 通过；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（53 app + 39 core 通过，2 个真实桌面测试忽略）、`git diff --check` 与上下文校验均通过。
- 残留风险：该任务没有将 `desktop_shell` 的同步/异步 OCR 入口迁入 `JobSupervisor`，因此 UI owner 关闭、asset generation 校验、并发限制与进程组回收仍须后续切片完成。
