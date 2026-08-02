# 任务 052：历史图像后台渲染预处理

- 状态：已完成
- 计划：`.context/plans/052_history_render_preprocessing.md`
- 规模：中
- 依赖：`.context/tasks/047_frame_cache_handoff.md`、`.context/tasks/049_history_editing.md`、`.context/tasks/051_history_async_loading.md`
- 生产行为变更：是；历史大图的预览、重新贴图和再次编辑避免在 UI 完成交付时执行整图像素转换。

## 范围

- 扩展 `HistoryLoadJobService`，使 worker 按预览、重新贴图、再次编辑三类意图准备最小像素结果。
- 让历史窗口缓存 worker 输出的 XRGB；让历史贴图和编辑 Overlay 复用 worker 输出，不再重复转换。
- 为准备输出、取消、陈旧结果和历史动作接入补充离线契约与回归测试。

## 任务目标

让历史图片在读取完成后仍保持主事件循环可立即处理关闭、选择、搜索及其他用户输入，避免大图的像素转换在 UI 中造成明显卡顿。

## 非目标

- 不改变历史窗口布局、工具、热键、文件数据格式或窗口任务栏/Dock 策略。
- 不实现缩略图索引、GPU 合成、跨显示器 Overlay、录屏或真实帧率基准。

## 预期文件

- `crates/pinora-app/src/{history_load_job.rs,history_window.rs,desktop_shell.rs}`
- `AGENTS.md`
- `.context/plans/052_history_render_preprocessing.md`
- `.context/tasks/052_history_render_preprocessing.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. worker 对预览、重新贴图、编辑分别只准备所需像素，预览完成消息不包含完整 RGBA 图像。
2. 历史完成处理不再调用全图 RGBA 到 XRGB/暗化转换；贴图、编辑与预览复用 worker 输出。
3. 051 的条目、owner、job ID、generation、当前选择、取消、关闭和帧缓存恢复语义不回退。
4. 服务/像素准备、历史浏览与 desktop shell 定向测试，以及严格 workspace 门禁和 ctx validate 通过。

## 验证

- `cargo test -p pinora-app history_load_job::tests -- --nocapture`
- `cargo test -p pinora-app history_window::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：准备结果携带多个全图缓冲，编辑大图峰值内存上升。缓解：按意图生成，预览不传递 RGBA、不生成暗化底图；维持单 worker。
- 风险：准备与消费意图不匹配会造成错误的贴图或预览。缓解：使用穷尽结果类型、现有 job 门禁与新增契约测试。
- 风险：worker 计算仍可能占用 CPU。缓解：不在 UI 线程执行；真实桌面性能探针不在本任务伪造。
- 回滚：回退本任务的 worker 准备结果和消费改动，恢复 051 的 UI 线程转换；不影响历史索引、PNG 或用户数据。

## 完成记录

- 2026-08-02：实现按预览、重新贴图、编辑区分的 worker 输出；预览不传递 RGBA，贴图和编辑分别复用预先生成的 XRGB 与 base/dimmed。取消、陈旧结果、owner/条目/generation/当前选择门禁及编辑失败后的帧缓存恢复路径保持不变。
- 已新增预览、贴图、编辑三类准备结果和单通道 XRGB 转换的离线测试；本地通过 fmt、workspace check、严格 Clippy、全量 workspace 测试（app 135 通过、2 个真实桌面测试忽略；core 55 通过）及 `git diff --check`。
- ctx validate 已通过；GitHub Actions CI `30734154282` 已在 Linux、macOS、Windows 原生 runner 通过格式、workspace 编译、严格 Clippy 与单元测试。真实慢盘、GUI 帧率、HiDPI、无障碍和原生窗口探针未覆盖，CI 不能替代这些验证。
