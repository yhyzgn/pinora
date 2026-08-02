# 任务 061：封闭无托盘 GUI 旁路

- 状态：已完成
- 计划：`.context/plans/061_tray_only_window_boundary.md`
- 规模：中
- 依赖：`.context/tasks/054_auxiliary_window_boundary.md`、`.context/tasks/058_tray_residency_capture_failures.md`、`.context/tasks/060_pin_context_menu_editing.md`
- 生产行为变更：是；移除可绕过系统托盘生命周期的遗留公开 GUI API。

## 任务目标

使 `run_desktop_shell` 成为唯一的公开 GUI 会话入口，删除仓库内部未使用且可启动独立 Overlay、区域工作流或贴图会话的导出，同时用测试锁定事件循环和窗口创建边界。

## 范围

- 检索并删除未被仓库使用的 `run_pin_session`、`PinView`、`PinSessionEnd`、`run_region_selection`、`capture_region_interactive` 及其专属实现。
- 保留并迁移 `desktop_shell` 所需的纯 `scaled_window_size` 计算，不让它携带独立事件循环。
- 在 `window_policy` 添加可离线执行的源代码门禁，确保生产源代码只有该模块构造事件循环和调用原始 `create_window`。
- 更新模块导出、计划、任务与系统风险记录。

## 非目标

- 不新增平台窗口属性，不改变 Overlay、贴图、OCR、导出、截图或菜单的用户交互。
- 不将源代码门禁、CI 或无显示测试视为真实任务栏/Dock、KWin、Wayland、macOS Dock 或 Windows shell 的验收。

## 预期文件

- `crates/pinora-app/src/{lib.rs,pin_window.rs,region_overlay.rs,region_workflow.rs,window_policy.rs,desktop_shell.rs}`
- `AGENTS.md`
- `.context/plans/061_tray_only_window_boundary.md`
- `.context/tasks/061_tray_only_window_boundary.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. `pinora-app` 不再导出或保留可独立启动无 tray GUI 会话的 API。
2. 贴图尺寸计算继续供 `desktop_shell` 使用，且其行为测试保持通过。
3. 测试失败于新增的生产事件循环构造或直接 `create_window` 调用。
4. fmt、workspace check、严格 Clippy、全量离线测试、diff 检查和 `ctx validate` 均通过。

## 验证

- `rg -n "run_pin_session|run_region_selection|capture_region_interactive|EventLoop::builder|\.create_window\(" crates/pinora-app/src`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：外部用户若直接依赖当前 `pinora-app` GUI 导出会遇到编译不兼容。缓解：该库尚未声明稳定 SDK；公开主流程仍由 `run_desktop_shell` 提供，发布说明须明确此次边界收紧。
- 风险：静态门禁不能捕获未来宏生成的窗口路径。缓解：禁止为本任务引入宏窗口工厂，并保留代码审查检索。
- 回滚：恢复遗留导出将重新违反 tray-only 约束；只有同时纳入受托盘会话时才能恢复其功能。

## 完成记录

- 已完成：引用审计确认 `run_pin_session`、`PinView`、`PinSessionEnd`、`run_region_selection`、`capture_region_interactive` 仅在自身模块或相互调用，未被根二进制、桌面壳或测试执行路径使用；已删除其无 tray 事件循环实现和公开导出。
- 已完成：`scaled_window_size` 迁至纯 `pin_layout` 模块，桌面壳继续使用原计算结果，不再依赖贴图窗口会话模块。
- 已完成：新增 `window_policy::tests::only_window_policy_may_construct_event_loops_or_windows`，扫描生产 Rust 源，拒绝任何非策略模块的事件循环构造或直接建窗。
- 已验证：`cargo test -p pinora-app window_policy::tests -- --nocapture`、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check`、`ctx validate` 均通过。
- 已知风险：源码级收敛不等同于真实操作系统窗口管理器的最终行为；四类原生桌面会话的 tray、任务栏/Dock、KWin、HiDPI 和输入性能验证仍开放。
