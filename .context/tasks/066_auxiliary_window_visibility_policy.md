# 任务 066：辅助窗口唯一可见性入口

- 状态：已完成
- 计划：`.context/plans/066_auxiliary_window_visibility_policy.md`
- 规模：中
- 依赖：`.context/tasks/050_tray_only_desktop_session.md`、`.context/tasks/054_window_policy_boundary.md`、`.context/tasks/061_tray_only_window_boundary.md`
- 生产行为变更：是；辅助窗口从创建到映射的时序改为统一策略入口。

## 任务目标

确保 Pinora 进程空闲时只存活于系统 tray。任何用户触发的遮罩层、截图 Overlay、贴图、历史、设置或编辑层都不得创建任务栏/Dock 新条目；所有可见映射必须经 `window_policy`，并在映射后立即执行平台隔离请求。

## 范围

- 强制 `create_auxiliary_window` 以隐藏状态创建；增加唯一的受策略保护展示入口。
- 迁移所有 Overlay、贴图、设置、历史和贴图恢复的可见映射调用。
- 递归扩展 `window_policy` 源码守卫，禁止策略模块外直接创建事件循环/窗口或直接请求可见。
- 更新计划、任务、系统事实和风险记录。

## 非目标

- 不扩展 tray、贴图、Overlay、截图、OCR、导出或窗口管理器脚本功能。
- 不承诺标准 Wayland 的通用任务栏隔离协议，也不伪造真实 Windows/macOS/X11/KDE Wayland 验收。

## 预期文件

- `crates/pinora-app/src/{window_policy.rs,desktop_shell.rs,history_window.rs,settings_window.rs}`
- `AGENTS.md`
- `.context/plans/066_auxiliary_window_visibility_policy.md`
- `.context/tasks/066_auxiliary_window_visibility_policy.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 工厂创建所有辅助窗口时保持不可见；只有 `window_policy` 能调用 `set_visible(true)`，并在同一入口应用映射后策略。
2. Overlay、贴图、设置、历史、贴图批量恢复和编辑恢复均没有直接可见映射绕过。
3. display handle 保持隐藏，且不存在新增窗口、事件循环、平台菜单或后台任务。
4. 源码守卫递归检查 `src/`，拒绝策略模块外的 `EventLoop::builder`、`.create_window(`、`.with_visible(true)`、`.set_visible(true)`。
5. 定向与全量离线门禁通过；真实任务栏/Dock、tray、合成器映射、首帧和 HiDPI 验证缺口明确保留。

## 验证

- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app history_window::tests -- --nocapture`
- `cargo test -p pinora-app settings_window::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：延后可见映射可能改变 Surface 首帧、焦点或恢复时序。缓解：先初始化 Surface，再经唯一入口显示；覆盖 Overlay、贴图、历史和设置定向测试。
- 风险：递归源码守卫误判测试或文档文本。缓解：只检查 Rust 源文件，策略模块本身是唯一允许位置。
- 风险：平台仍可忽略隔离请求。缓解：保留真实桌面验证为开放风险，不以静态门禁代替。
- 回滚：移除唯一展示入口、恢复既有局部调用；不改变 tray、截图、贴图数据或公共接口。

## 完成记录

- 已实现：`create_auxiliary_window` 强制使用隐藏属性；`show_auxiliary_window` 是 Overlay、贴图、设置和历史窗口的唯一可见映射入口，并在映射后执行 KWin 的任务栏/分页器隔离。所有贴图恢复路径均保留稳定窗口标题并重新走该入口。
- 已实现：`window_policy` 测试递归扫描 Rust 源，拒绝策略模块外的 `EventLoop::builder`、`.create_window(`、`.with_visible(true)`、`.set_visible(true)`；display handle 保持不可见。
- 已验证：`cargo test -p pinora-app window_policy::tests -- --nocapture`（4 通过）、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`（25 通过）、`cargo test -p pinora-app history_window::tests -- --nocapture`（0 个匹配用例）、`cargo test -p pinora-app settings_window::tests -- --nocapture`（0 个匹配用例）、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 170 通过/2 忽略，core 71 通过）、`git diff --check` 和上下文校验。
- 未覆盖风险：离线源码守卫与单元测试不能证明真实 Windows/macOS/X11/KDE Wayland 的任务栏/Dock、tray、合成器映射时机、首帧、焦点或 HiDPI 行为；这些必须在原生桌面会话验证。
