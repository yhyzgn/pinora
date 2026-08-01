# 计划 022：OCR 子进程生命周期

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/022_ocr_process_lifecycle.md`

## 目标

将本地 `tesseract` CLI 适配器改为由自身持有 `Child` 并进行受控等待、协作式取消、超时回收和临时文件自动清理，移除外部 `kill -9` 进程控制。

## 非目标

- 不迁移 `desktop_shell` 的 OCR 触发、结果展示或贴图 owner 生命周期。
- 不改 OCR 引擎、语言选择、TSV 解析或模型分发策略。
- 不实现任务队列、进度事件、并发限流、OCR 缓存或真实桌面端到端探针。

## 约束

- 适配器只终止自己启动并持有的子进程；不得按 PID 调用外部 `kill` 命令。
- 取消和超时使用稳定错误码；退出前必须等待子进程被回收。
- 临时 PNG 无论 OCR 成功、失败、取消或超时都应由 RAII 清理。
- 新 API 只能接收 `JobCancellation` 的只读协作式令牌，不能把子进程句柄泄漏给领域或 UI。

## 依赖关系

- 依赖 021 的 `JobCancellation` 契约；本任务只使用令牌，不把旧 UI 接入监督器。
- 依赖 019 的资产版本契约，但本轮不产生/提交 `JobResultRef`。

## 阶段

1. 提取可测试的子进程等待/回收边界，保留现有 `recognize_image` 兼容入口。
2. 支持受监督令牌取消和固定超时，使用 `Child::kill`/`wait` 回收自身进程。
3. 为取消、超时、临时文件清理和 TSV 上限建立本地、无 OCR 引擎依赖的测试。

## 检查点

- `ocr.rs` 不再出现 `Command::new("kill")` 或后台 `wait_with_output` 线程。
- 无论结果如何，已启动 child 均由同一调用栈执行 `wait` 回收。
- 默认同步 API 行为保持，新增可取消 API 仅为后续监督器接入提供入口。

## 计划级风险

- 子进程的子孙进程在平台上可能需要进程组策略；当前 `tesseract` 单进程路径只保证直接 child 回收，复杂引擎包装留待平台适配任务验证。
- 本地命令测试只能证明 `Child` 回收协议，不能证明 Tesseract 模型准确率或桌面 OCR 交互。

## 完成标准

- OCR 适配器对取消和超时返回稳定错误码，且不使用外部 PID kill。
- 覆盖受控 child 取消、超时、输出上限和临时 PNG 自动清理的离线测试。
- fmt、check、严格 Clippy、workspace 测试、差异检查和上下文校验通过。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：OCR 执行与语言探测均由适配器持有 `Child`，通过 `try_wait`、协作式 `JobCancellation`、截止时间和 `Child::kill`/`wait` 完成回收；已移除外部 `kill` 命令与无归属 `wait_with_output` 线程。临时 PNG 改由 `TempPng` 的 `Drop` 清理，stdout/stderr 各限制为 16 MiB。
- 错误语义：新增 `cancelled`、`timed_out`、`resource_limit_exceeded` 稳定错误码；保留 `recognize_image`，新增 `recognize_image_with_cancellation` 供后续监督器接入。
- 验证：OCR 定向测试 8/8、错误码测试 1/1、`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（53 app + 39 core 通过，2 个真实桌面测试忽略）、`git diff --check` 和上下文校验通过。
- 残留风险：`desktop_shell` 仍直接触发 OCR，尚未创建 `JobSpec`、关闭 owner 或经 `JobSupervisor` 审核结果；直接 child 回收不覆盖复杂引擎自行派生的子孙进程。
