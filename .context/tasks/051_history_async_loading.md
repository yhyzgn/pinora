# 任务 051：历史图像异步加载与陈旧结果隔离

- 状态：已完成
- 计划：`.context/plans/051_history_async_loading.md`
- 规模：中
- 依赖：`.context/tasks/021_job_supervision_contract.md`、`.context/tasks/042_history_browser.md`、`.context/tasks/049_history_editing.md`、`.context/tasks/050_tray_only_windows.md`
- 生产行为变更：是；历史窗口在加载大图时保持响应，取消或切换条目不会交付旧结果。

## 范围

- 新增可注入的历史图像加载应用服务，复用 `JobSupervisor`、取消令牌和 worker 收敛工具。
- 扩展任务身份以表达历史读取；为历史预览、重新贴图和再次编辑建立显式加载意图与主线程交付分支。
- 更新历史窗口加载中状态；在选择变化、关闭、删除、清空、截图切换和退出路径取消请求。
- 保留并在 worker 内复用 `load_history_image` 的所有文件与 PNG 完整性校验。

## 任务目标

使历史窗口在慢速读取或大 PNG 解码期间仍能处理输入和关闭操作，同时确保只有当前选中的条目可以生成预览、贴图或编辑 Overlay。

## 非目标

- 不做缩略图持久化、标签、历史列表虚拟化、窗口截图、录屏、点击穿透或 UI 框架替换。
- 不改变历史文件来源、索引 codec、导出成功语义或 049 的编辑工作流。

## 预期文件

- `crates/pinora-core/src/job.rs`
- `crates/pinora-app/src/{lib.rs,history_export.rs,history_load_job.rs,history_browser.rs,history_window.rs,desktop_shell.rs}`
- `AGENTS.md`
- `.context/plans/051_history_async_loading.md`
- `.context/tasks/051_history_async_loading.md`
- `.context/system/{overview.md,risks.md,conventions.md}`

## 验收标准

1. 历史选择、输入搜索和 `Reopen`/`Edit` 事件路径不直接调用 `load_history_image`；加载由 worker 完成。
2. 服务拒绝种类、历史条目身份、owner 或当前选择不匹配的结果；关闭窗口、选择切换和退出会取消对应 worker。
3. 成功预览只更新当前条目；成功重新贴图/编辑只执行原有主线程逻辑；失败保留历史窗口，取消/陈旧不显示误导性错误。
4. 历史完整性校验、重新贴图和编辑的既有行为不回退；严格本地质量门禁与 GitHub CI 通过。

## 验证

- `cargo test -p pinora-core job::tests -- --nocapture`
- `cargo test -p pinora-app history_load_job::tests -- --nocapture`
- `cargo test -p pinora-app history_export::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：磁盘读或 PNG 解码不可抢占；取消后完成的结果仍须经门禁丢弃，退出仅做有界等待。
- 风险：扩展任务身份遗漏匹配分支；通过穷尽匹配、服务契约和严格 Clippy 发现。
- 风险：编辑 worker 完成后打开 Overlay 失败导致帧缓存暂停；沿用 049 失败恢复路径并补回归测试。
- 回滚：移除异步服务与 shell 编排，恢复同步读取；历史索引、PNG 和用户数据不变。

## 完成记录

- 2026-08-02：新增 `history_load_job`，使用可注入 runner、`JobSupervisor`、取消令牌和有界 worker 回收。服务拒绝错误任务种类、历史条目身份不匹配、当前选择缺失和 generation 变化的结果，并锁定单 worker 限流。
- `desktop_shell` 已删除所有同步 `load_history_image` 调用；预览、Enter 贴图和 `E` 编辑只入队最新历史请求，读取完成后才在主线程更新预览、创建贴图或打开编辑 Overlay。
- 选择、搜索、关闭、删除、清空、配额清理、截图切换与退出均取消历史读取；取消或陈旧结果不可更新 UI。面板以 `Loading` 表示正在读取，失败保留历史窗口并使用既有错误状态。
- 已通过 `history_load_job::tests`、`history_browser::tests`、`history_export::tests`、`desktop_shell::overlay_scale_tests`、fmt、workspace check、严格 Clippy、全量 workspace 测试（app 131 通过、2 个真实桌面测试忽略；core 55 通过）、`git diff --check` 与 ctx validate。真实慢盘、GUI/HiDPI/无障碍与跨平台桌面探针未覆盖；预览色彩转换和历史编辑帧准备仍有主线程像素工作，未被描述为已消除。
- GitHub Actions CI `30733684203` 已在 Linux、macOS、Windows 原生 runner 通过格式、workspace 编译、严格 Clippy 与单元测试；该结果不覆盖真实桌面窗口、慢盘、输入响应或渲染帧延迟。
