# 任务 073：Overlay 选区物理像素读数

- 状态：已完成
- 计划：`.context/plans/073_selection_metrics_readout.md`
- 规模：小
- 依赖：`.context/tasks/009_region_overlay.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/066_auxiliary_window_visibility_policy.md`、`.context/tasks/072_selection_resize_handles.md`
- 生产行为变更：是；当前 Overlay 显示选区物理像素尺寸与全局坐标。

## 任务目标

在不新增窗口、截图或后台任务的前提下，在已有 Overlay 内显示当前选区的源图物理像素宽高和全局左上坐标；读数在拖选、键盘微调及八方向调整期间与现有节流一致更新，并在重选/取消时无残影。

## 范围

- 增加选区读数的纯映射、布局和帧内绘制。
- 接入既有 Overlay 脏区和刷新路径。
- 覆盖缩放映射、负 origin、边界/工具栏避让、重选与热区调整回归。
- 更新计划、任务、系统事实和风险记录。

## 非目标

- 不新增窗口、提示框、Toast、系统菜单、显示器菜单或可见性调用。
- 不改截图、标注、贴图、导出、OCR、历史、设置、`DisplayId` 或 `window_policy`。

## 预期文件

- `crates/pinora-app/src/desktop_shell.rs`
- `crates/pinora-app/src/overlay_selection_readout.rs`
- `crates/pinora-app/src/lib.rs`
- `crates/pinora-app/src/settings_panel.rs`
- `AGENTS.md`
- `.context/plans/073_selection_metrics_readout.md`
- `.context/tasks/073_selection_metrics_readout.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 读数使用源图物理像素：尺寸与 `buf_rect_to_src` 一致，坐标为 `display_origin + source_rect.origin`，包括负 origin。
2. 读数始终在既有 Overlay bounds 内，与工具栏和选区边框保持间距；拖选、反向拖选、键盘移动、热区调整、重选和取消不留残影。
3. 不创建、显示或管理新窗口，也不启动事件循环、系统菜单、截图或 worker；空闲 Pinora 继续只由 tray 驻留，辅助层禁止任务栏、Dock 或分页器项。

## 验证

- `cargo test -p pinora-app overlay_selection_readout -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：显示了缓冲/逻辑尺寸而非原图物理像素。缓解：只接受既有映射函数输出，测试缩放和负 origin。
- 风险：读数与工具栏重叠或残留旧像素。缓解：纯布局优先避让工具栏，并将新旧 bounds 都纳入脏区恢复。
- 风险：增加高频呈现负担或绕开 tray-only 窗口策略。缓解：复用既有拖动节流和 Overlay 帧，不添加窗口/worker，并执行源码守卫。
- 回滚：删除读数模块及其 Overlay 调用，恢复原有选区绘制和工具栏；不影响截图、标注、贴图、导出、OCR、tray 或窗口策略。

## 完成记录

- 已完成：`overlay_selection_readout` 在现有 Overlay 帧中绘制 `W… H… X… Y…` 读数。尺寸来自源图物理像素，X/Y 先通过 `buf_rect_to_src` 映射、再叠加 `display_origin`；缩放缓冲和负全局 origin 均有回归覆盖。
- 已完成：读数面板优先放置在选区上方，工具栏位于下方时不重叠；极小画布下仍被限定在当前 Overlay 内。新旧面板区域都由 `dimmed` 帧恢复，读数随普通拖选、键盘移动和八方向热区调整在既有 2 像素/32ms 节流下刷新，不会引入额外窗口、事件循环、系统菜单、截图或 worker。
- 已完成：为共享点阵字体补全 `X` 字形，仅服务现有帧内物理坐标读数；未改变任何窗口、数据或后台任务接口。
- 已验证：`cargo test -p pinora-app overlay_selection_readout -- --nocapture`、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`、`cargo test -p pinora-app window_policy::tests -- --nocapture`、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check` 与 `ctx validate` 通过；全量离线结果为 app 182 通过、2 忽略，core 84 通过。
- 未验证：离线测试无法证明真实 Windows、macOS、X11、KDE Wayland 中的读数可读性、HiDPI 命中、连续拖动帧时间、焦点、任务栏/Dock/分页器隔离或 tray 连续驻留；这些仍需原生会话验收。
