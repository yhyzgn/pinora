# 任务 036：实现 OCR 文字层拖选与复制

- 状态：已完成
- 计划：`.context/plans/036_ocr_text_selection.md`
- 规模：中
- 依赖：`.context/tasks/023_ocr_job_service.md`、`.context/tasks/026_export_clipboard_job_service.md`
- 生产行为变更：是；贴图窗口新增 Ctrl+左键文字选择和复制动作。

## 变更前记录

```text
目的：让 OCR 词框可选中并复制局部文本，而不是只能全文复制。
影响路径：pinora-core OCR 领域模型、pinora-app desktop_shell 贴图事件/绘制、上下文文档。
兼容性：保留普通左键拖动、滚轮缩放、O/T/Esc 和现有 OCR 全文复制语义；不改变 OCR 输出格式。
外部副作用：选中文本通过既有本地剪贴板 adapter 异步写入；不连接共享服务，不记录全文日志。
回滚点：移除选择字段和 Ctrl+拖拽分支即可恢复原贴图行为。
验证场景：跨行选择顺序、空选择不提交、缩放映射、选中框高亮、贴图关闭后任务丢弃。
```

## 任务目标

新增稳定的 `OcrTextSelection` 词引用与文本导出 API，在贴图窗口接入拖选、选中框高亮和监督文本复制。

## 范围

- `crates/pinora-core/src/ocr.rs` 与导出符号。
- `crates/pinora-app/src/desktop_shell.rs` 的贴图状态、输入映射、绘制和任务提交。
- 计划、任务、系统概览、风险和 `AGENTS.md` 工作指针。

## 预期文件

- `crates/pinora-core/src/ocr.rs`、`lib.rs`。
- `crates/pinora-app/src/desktop_shell.rs`。
- `.context/plans/036_ocr_text_selection.md`、`.context/tasks/036_ocr_text_selection.md`、`.context/system/overview.md`、`.context/system/risks.md`。

## 非目标

- OCR 引擎替换、模型下载、富文本编辑、Overlay 文字层和 OCR 历史持久化。

## 验收标准

- 选择矩形与词框相交的词按行/词顺序生成文本；越界词引用被安全忽略。
- Ctrl+拖拽只在 OCR 结果存在且词框显示时进入文字选择；普通拖动不回归。
- 复制任务带当前 `AssetRef`/owner，贴图关闭或 generation 变化时不会发布系统复制成功。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-core ocr::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：新增手势与窗口拖动竞争；通过 Ctrl 修饰键门禁和现有普通拖动测试隔离。
- 风险：缩放换算取整导致边缘词选择偏差；通过半开区间矩形和缩放单测覆盖。
- 回滚：移除 `OcrTextSelection` 与贴图选择字段/分支，不改 OCR worker 和剪贴板适配器。

## 完成记录

- 状态：已完成（2026-08-02）。
- 实际变更：OCR core 选择引用按行/词排序并去重，拖拽矩形相交词框才进入选择，局部文本不进入日志。
- 实际变更：desktop shell 为 `PinWin` 增加选择状态、拖拽预览和高亮；Ctrl+左键与普通窗口拖动分离，释放时提交受监督文本复制任务。
- 验证：`cargo test -p pinora-core ocr::tests -- --nocapture` 4/4、desktop shell 9/9、workspace check/Clippy/tests、fmt/diff/ctx 全部通过；app 90、core 52，2 个真实桌面测试忽略。
- 未覆盖项：真实 GUI、系统剪贴板、Wayland/X11 多窗口管理器和 OCR 引擎分词差异探针；富文本编辑仍属后续任务。
