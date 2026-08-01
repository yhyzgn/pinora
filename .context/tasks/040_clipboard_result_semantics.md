# 任务 040：修正系统剪贴板结果语义

- 状态：已完成
- 计划：`.context/plans/040_clipboard_result_semantics.md`
- 规模：中
- 依赖：`.context/tasks/011_system_clipboard.md`、`.context/tasks/025_clipboard_process_lifecycle.md`、`.context/tasks/027_desktop_export_job_integration.md`
- 生产行为变更：是；同步图像复制在系统剪贴板失败时返回明确失败，不再伪造成功。

## 变更前记录

```text
目的：区分内存剪贴板缓存成功与系统剪贴板写入成功。
影响路径：ErrorCode、LocalImageSink、同步 AppRuntime 测试和上下文事实。
兼容性：保留 ImageSink 接口、内存副本和桌面异步 ExportJobService；改变无系统剪贴板时同步 copy_image 的返回结果。
外部副作用：只调用既有本地 wl-copy/xclip/xsel 适配器；不连接共享服务。
回滚点：恢复 LocalImageSink 的成功/日志分支，保留错误码与测试以便重试迁移。
验证场景：系统写入成功、后端不可用、命令失败、编码失败、内存副本可重试。
```

## 任务目标

让 `LocalImageSink::copy_image` 在所有路径发布准确的系统剪贴板结果：内存副本先保留；PNG 编码或系统写入失败返回 `ClipboardFailed` 或原始稳定错误；日志不暴露路径和内容。

## 范围

- `crates/pinora-core/src/error.rs` 新增稳定剪贴板失败错误码。
- `crates/pinora-app/src/image_sink.rs` 修改同步图像复制结果语义与测试。
- `crates/pinora-app/src/runtime.rs` 同步复制调用方/测试适配。
- 当前工作指针、上下文事实和风险记录。

## 预期文件

- `crates/pinora-core/src/error.rs`。
- `crates/pinora-app/src/image_sink.rs`、`crates/pinora-app/src/runtime.rs`。
- `AGENTS.md`、`.context/plans/040_clipboard_result_semantics.md`、`.context/tasks/040_clipboard_result_semantics.md`、`.context/system/overview.md`、`.context/system/risks.md`。

## 非目标

- 不迁移已有异步剪贴板 worker，不改变 OCR 文本复制的内容或任务状态字符串。
- 不实现 Windows/macOS 原生剪贴板，不执行真实 GUI 或系统剪贴板验收。

## 验收标准

- 系统 PNG 写入成功才返回 `Ok(())`；后端缺失/命令失败返回 `ErrorCode::ClipboardFailed`。
- 任何系统失败后 `clipboard_image_id`/缓存字节仍可查询，重新调用可重试；失败日志不含命令路径、像素或 OCR 全文。
- 所有同步调用方处理新失败语义，workspace 质量门禁通过。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-core error::tests -- --nocapture`
- `cargo test -p pinora-app image_sink::tests -- --nocapture`
- `cargo test -p pinora-app runtime::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：无系统剪贴板的 CLI/测试环境会从成功变为失败。缓解：内存缓存保持，调用方按 `ClipboardFailed` 显示可重试提示；fake sink 继续覆盖领域流程。
- 风险：错误日志泄露平台命令路径。缓解：向调用方只映射稳定错误码，原始适配器详情仅保留进程内诊断且不输出。
- 回滚：恢复 `LocalImageSink` 的返回映射，不撤销异步 ExportJobService 监督和错误码契约。

## 完成记录

- 状态：已完成（2026-08-02）。
- 实际变更：`ErrorCode` 新增稳定 `clipboard_failed` 字符串；`LocalImageSink::copy_image` 的内存缓存与系统剪贴板结果分离，编码/写入失败返回 `ClipboardFailed`，缓存仍可查询和再次调用。
- 实际变更：同步 runtime 测试适配真实环境的成功/失败双路径；私有系统写入器注入测试在不依赖桌面会话的情况下证明失败保留图像，失败日志不含命令路径、像素或 OCR 全文。
- 验证：`cargo fmt --check`；`cargo test -p pinora-core error::tests -- --nocapture`（1/1）；`cargo test -p pinora-app image_sink::tests -- --nocapture`（8/8，1 个真实剪贴板测试忽略）；`cargo test -p pinora-app runtime::tests -- --nocapture`（11/11）；workspace check、严格 Clippy、workspace 测试（152 通过，2 忽略）、diff 检查，均通过。
- 未覆盖风险：真实系统剪贴板读回、平台原生 adapter 和用户可见重试 UI 未验证；桌面异步复制路径保持原有任务监督边界。
