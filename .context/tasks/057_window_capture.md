# 任务 057：托盘窗口截图与安全候选快照

- 状态：进行中
- 计划：`.context/plans/057_window_capture.md`
- 规模：大
- 依赖：`.context/tasks/054_auxiliary_window_boundary.md`、`.context/tasks/055_display_targeted_capture.md`、`.context/tasks/056_delayed_capture.md`
- 生产行为变更：是；tray 在后端可用时新增经清洗的窗口截图候选，成功后在既有标注 Overlay 打开真实窗口图像。

## 范围

- 定义窗口候选、稳定目标身份和窗口捕获请求，扩展所有 `CaptureProvider` 实现的明确支持/拒绝路径。
- 在 xcap 后端实现候选枚举、Pinora 窗口过滤、重新验证及 `capture_image` 到 `CaptureImage` 的受控转换。
- 将候选映射接入 tray 与 desktop shell 冷捕获/Overlay 流程；不创建候选浮窗、控制窗或任何绕过 `window_policy` 的窗口。
- 为契约、目标失效、菜单标签过滤、后端拒绝和 tray 映射增加离线测试，并更新经过验证的上下文和风险。

## 任务目标

在不创建控制窗口、候选浮窗或任务栏/Dock 入口的前提下，从 tray 选中一个已验证的非 Pinora 窗口并将其真实像素交接给既有标注 Overlay；任何失效或平台失败都保持进程仅以 tray 常驻。

## 非目标

- 不做鼠标高亮选窗、窗口阴影裁剪、最小化窗口恢复、窗口列表自动刷新、定时窗口截图或新的全局热键。
- 不改造现有区域、全屏、显示器和延时截图，也不改变历史、OCR、贴图、导出或设置存储。

## 预期文件

- `crates/pinora-core/src/{capture.rs,ids.rs,lib.rs}`（仅在实际公开契约需要时）
- `crates/pinora-app/src/{capture_fake.rs,capture_kde.rs,capture_select.rs,capture_xcap.rs,tray.rs,desktop_shell.rs}`
- `AGENTS.md`
- `.context/plans/057_window_capture.md`
- `.context/tasks/057_window_capture.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 候选窗口与后台内部 ID 分离，标签清洗且不含 Pinora 自己窗口；候选消失或拓扑变化时明确失败且不回退。
2. xcap 返回的窗口像素、几何、显示器和缩放经过验证后成为真实 `CaptureImage`；未支持后端返回 `CapabilityUnavailable`。
3. tray 点击在冷捕获完成后复用现有 Overlay；取消、失败和关闭回到 tray 常驻，不创建新的任务栏/Dock 表面。
4. 所有 `CaptureProvider` 实现和调用方已更新并由定向离线测试覆盖。
5. 定向测试、fmt、workspace check、严格 Clippy、全量测试、diff 检查、`ctx validate` 与 GitHub 三平台 CI 通过。

## 验证

- `cargo test -p pinora-core capture -- --nocapture`
- `cargo test -p pinora-app capture_xcap::tests tray::tests desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：不同平台/合成器的窗口列举或像素捕获能力不一致。缓解：按后端明确拒绝，永不以屏幕图像替代窗口图像。
- 风险：菜单快照过期或标题重用。缓解：捕获前按内部 ID、几何与显示器重新验证；不匹配即 `NotFound`。
- 风险：候选中包含 Pinora 或标题泄露。缓解：按应用名/标题过滤 Pinora，菜单文本截断，日志只记录受控结果而不打印候选标题或 ID。
- 回滚：删除窗口候选菜单和相关捕获分支；现有截图、窗口策略和用户数据不受影响。

## 完成记录

- 已实现：窗口候选的内部 ID 与本地菜单文本分离，候选标签清洗/截断且最多 20 项；xcap 在实际取像前按 ID、几何、显示器、缩放和最小化状态重验，任何不匹配均明确失败且不回退为显示器截图。所有成功 Overlay 继续经 `window_policy` 创建；窗口捕获的同步启动、worker 错误、worker 断开、渲染缓冲不一致和 Overlay 创建失败均返回 tray 空闲态。
- 已验证：`cargo fmt`；`cargo test -p pinora-core capture -- --nocapture`（6 通过）；`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`（21 通过）；`cargo test -p pinora-app tray::tests -- --nocapture`（6 通过）；`cargo test -p pinora-app capture_xcap::tests -- --nocapture`（1 通过、1 个真实桌面测试忽略）；`cargo check --workspace` 与 `cargo clippy -p pinora-core -p pinora-app --all-targets -- -D warnings` 通过。
- 待验证：全量 workspace 门禁、`ctx validate`、本次 GitHub 三平台 CI，以及真实 Windows/macOS/X11/KDE Wayland 桌面会话中的窗口候选、权限、像素、Overlay 与任务栏/Dock 行为。
