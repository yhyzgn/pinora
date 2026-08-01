# 任务 028：实现原子 PNG 导出

- 状态：已完成
- 计划：`.context/plans/028_atomic_png_export.md`
- 规模：中
- 依赖：`.context/tasks/025_clipboard_process_lifecycle.md`、`.context/tasks/027_desktop_export_job_integration.md`
- 生产行为变更：是；PNG 文件只会在完整编码、同步和原子替换后出现在目标路径，失败时保留旧目标文件。

## 目的

修复 `LocalImageSink::save_png` 对目标路径直接 `File::create` 的风险，避免导出失败或中断时覆盖用户已有文件为不完整 PNG。

## 任务目标

新增同目录的 RAII 临时导出文件：以唯一 `create_new` 创建，写入 PNG 后 flush、取回 `File` 并 `sync_all`，再 rename 到目标，最后打开目标验证可读；未完成或失败时 Drop 删除临时文件。

## 影响路径

- `crates/pinora-app/src/image_sink.rs` 的 PNG 保存实现和离线测试。
- 当前计划、任务、`.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 兼容性

- 接口：不改 `ImageSink::save_png`、`LocalExportRunner`、命令或事件接口。
- 数据/状态：不改图像像素、保存目录、稳定状态字符串、持久化或权限语义。
- 文件行为：Linux 保持覆盖既有目标的意图，但替换失败时返回错误并保留旧目标；临时文件仅存在于目标目录。

## 外部副作用

离线测试仅在系统临时目录创建、替换和删除测试 PNG；不访问真实桌面、网络或共享基础设施。

## 回滚点

恢复 `save_png_file` 的直接写入实现即可回退；不影响导出任务监督、剪贴板 child 生命周期或 OCR。

## 验证场景

- 新目标导出后包含 PNG 签名且可重新打开。
- 目标已经存在时，成功导出以完整 PNG 原子替换旧内容。
- 已创建但未提交的临时文件在 Drop 后被删除。
- 创建、编码、同步或替换失败时不发布成功，旧目标保持可用。

## 范围

- 新增同目录临时 PNG 文件工具、同步和替换逻辑。
- 更新既有 PNG 测试并补临时文件清理/替换测试。
- 更新上下文事实和风险。

## 非目标

- 不做跨平台 replace 适配、目录 fsync、断电恢复日志、格式扩展或文件命名策略。
- 不修改 `ExportJobService`、`desktop_shell`、UI 进度或系统剪贴板语义。
- 不把本地文件测试描述为真实 GUI E2E。

## 预期文件

- `crates/pinora-app/src/image_sink.rs`。
- `.context/plans/028_atomic_png_export.md`。
- `.context/tasks/028_atomic_png_export.md`。
- `.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- `save_png_file` 不直接创建最终目标文件；成功路径必须同步并校验可读。
- 未提交临时文件自动删除，成功替换后不存在残留。
- PNG 既有行为和所有质量门禁通过，平台/断电验证缺口清楚记录。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app image_sink::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `rg -n 'File::create\(path\)|rename\(' crates/pinora-app/src/image_sink.rs`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：同目录临时文件唯一性或 Drop 清理错误。缓解：`create_new`、进程/计数器唯一后缀和直接 Drop 单测。
- 风险：原子替换平台语义不一致。缓解：只记录 Linux 事实；失败时保留旧目标并返回错误，不隐式删除目标。
- 回滚：仅恢复直接写入；保留 025-027 的生命周期与服务改造。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：`save_png_file` 直接 `File::create(path)` 后编码，目标文件会在写入完成前被截断；未做 `sync_all`、临时文件清理、原子替换或目标可读性校验。
- 实际变更：新增 `AtomicPngTemp`，在目标同目录以 `create_new` 创建唯一临时文件；完整 PNG 写入并 `sync_all` 后关闭、rename 发布、打开验证可读，未提交实例 Drop 删除临时文件。`save_png_file` 不再直接创建目标路径。
- 验证：`cargo test -p pinora-app image_sink::tests -- --nocapture`（7/7、1 忽略）；workspace check、严格 Clippy、workspace 测试（71 app 通过/2 忽略，39 core 通过）、静态扫描、差异检查和上下文校验通过。
- 未覆盖项：未验证 Windows/macOS 或网络文件系统 rename，未 fsync 父目录，未运行真实桌面 E2E；退出 worker 收敛留待后续任务。
