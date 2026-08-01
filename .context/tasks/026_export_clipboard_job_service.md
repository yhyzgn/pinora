# 任务 026：建立导出与剪贴板受监督服务

- 状态：已完成
- 计划：`.context/plans/026_export_clipboard_job_service.md`
- 规模：大
- 依赖：`.context/tasks/021_job_supervision_contract.md`、`.context/tasks/025_clipboard_process_lifecycle.md`
- 生产行为变更：无；新增尚未接入旧 UI 的应用服务和 runner 契约。

## 目的

将保存 PNG、复制图像和复制 OCR 文本从单一 `ImageSink` 同步端口抽象为可监督后台任务，为后续桌面接入提供一致的 owner/generation 与失败语义。

## 任务目标

新增 `ExportJobService`、`ExportRunner`、`LocalExportRunner` 与不可变 `ExportJobInput`。提交时校验 `JobKind` 与输入匹配，worker 只执行 runner 并回送 `JobResultRef`；主线程轮询先过 `JobSupervisor`，再交付统一完成/失败/丢弃结果。

## 影响路径

- 新增 `crates/pinora-app/src/export_job.rs` 并在 `lib.rs` 导出公共服务/端口。
- `crates/pinora-app/src/image_sink.rs` 增加可复用的生产 runner 入口；不改变 `ImageSink` trait。
- 当前计划、任务、系统概览和风险登记。

## 兼容性

- 接口：只新增应用层类型；保留 `ImageSink`、`copy_text_to_system_clipboard` 和 `AppRuntime` 同步命令。
- 数据/状态：不修改 `AppState`、领域命令、事件字符串、持久化形状或权限语义。
- 外部副作用：fake runner 测试只启动 Rust worker；生产 runner 只有被显式调用才访问本地文件/剪贴板命令。

## 外部副作用

离线验证不调用共享服务、真实桌面或系统剪贴板；既有真实剪贴板测试继续保持 ignored。

## 回滚点

删除 `export_job.rs`、导出和测试即可回退；不回滚 025 的 child 生命周期修复，也不改变旧 `AppRuntime` 行为。

## 验证场景

- `Export` + PNG 输入成功完成；`Clipboard` + 图像/文本输入成功完成。
- runner 错误进入 `Failed`，不覆盖取消、owner 关闭或超时终态。
- 关闭 owner、截止时间到达或当前 generation 变化后，worker 结果被丢弃。
- 错误和完成事件不含图像像素、OCR 全文或窗口句柄。

## 范围

- 新增三类导出/剪贴板输入和统一服务完成协议。
- 实现生产 runner，复用 `LocalImageSink` 的 PNG/图像剪贴板与文本剪贴板能力。
- fake runner 契约测试和公共导出；更新上下文事实。

## 非目标

- 不将 `ExportJobService` 接入 `desktop_shell` 或 `AppRuntime`；不删除同步路径。
- 不实现进度、暂停、队列持久化、缓存、重试、用户取消 UI 或文件选择器。
- 不宣称真实系统剪贴板、GUI E2E 或子孙进程组回收已经验证。

## 预期文件

- `crates/pinora-app/src/export_job.rs`。
- `crates/pinora-app/src/image_sink.rs`、`crates/pinora-app/src/lib.rs`。
- `.context/plans/026_export_clipboard_job_service.md`。
- `.context/tasks/026_export_clipboard_job_service.md`。
- `.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- 服务只接受合法的 `JobKind`/输入组合，worker 不接触 UI；结果经 owner、generation、截止时间和终态门禁。
- fake runner 覆盖保存、复制图像、复制文本、失败、owner 关闭、超时和陈旧资产。
- 生产 runner 复用现有适配器，不新增依赖、警告抑制或真实外部调用；质量门禁全通过。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app export_job::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `rg -n 'winit|Window|AppRuntime|desktop_shell' crates/pinora-app/src/export_job.rs`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：服务 worker 只在 runner 入口检查取消，外部命令中途取消仍受 025 的 3 秒适配器上限约束。缓解：服务 API 始终传递 `JobCancellation`，后续 027 将取消接入事件循环。
- 风险：生产 runner 与旧 `LocalImageSink` 共享适配器逻辑但不共享内存剪贴板状态。缓解：本任务不改变旧同步语义，桌面迁移时再决定共享 store 和系统失败事件。
- 回滚：移除新增服务、runner、导出和测试；旧同步 API 与 025 子进程适配器保持不变。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：`AppRuntime::dispatch` 在事件线程同步调用 `ImageSink::save_png/copy_image`；OCR 完成事件直接调用 `copy_text_to_system_clipboard`；已有 `JobSupervisor` 只被 `OcrJobService` 使用，导出/剪贴板没有任务 owner 或 generation 门禁。
- 实际变更：新增 `ExportJobInput`、`ExportRunner`、`LocalExportRunner`、`ExportJobService` 与完成协议；生产 runner 复用 PNG 编码、文件保存和可取消系统剪贴板适配器，worker 不持有 UI 或 runtime。服务在提交前校验输入类型/图像 ID，在轮询时按 owner、资产 generation、截止时间和终态决定交付。
- 验证：`cargo test -p pinora-app export_job::tests -- --nocapture`（6/6）、`cargo test -p pinora-app image_sink::tests -- --nocapture`（5/5、1 忽略）、workspace check、严格 Clippy、workspace 测试（68 app 通过/2 忽略，39 core 通过）、静态扫描、差异检查和上下文校验通过。
- 未覆盖项：服务尚未接入 `desktop_shell`；旧同步 API 和 UI 关闭/退出路径未使用 owner 取消；文件原子导出、真实系统剪贴板和孙进程组回收仍待后续任务验证。
