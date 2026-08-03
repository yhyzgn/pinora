# 任务 100：Wayland GlobalShortcuts Portal

- 状态：已完成
- 计划：`.context/plans/100_wayland_global_shortcuts_portal.md`
- 规模：大
- 依赖：`GlobalHotkeyHub`、`DesktopApp` GUI 事件循环、现有安全 `HotkeyBinding`、tray/诊断能力摘要与 `window_policy`。
- 生产行为变更：是；Linux 纯 Wayland 可在系统 Portal 后端支持并授权后注册两项全局截图动作。

## 任务目标

实现独立的 XDG `GlobalShortcuts` Portal 适配器，并将其与已有全局热键中枢组合，在不阻塞 GUI、不创建常驻窗口且不影响 X11/IPC 回退的前提下支持纯 Wayland 热键。

## 范围

- `crates/pinora-app/Cargo.toml`
- `crates/pinora-app/src/wayland_portal.rs`
- `crates/pinora-app/src/hotkey.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `crates/pinora-app/src/lib.rs`
- `AGENTS.md`
- `.context/plans/100_wayland_global_shortcuts_portal.md`
- `.context/tasks/100_wayland_global_shortcuts_portal.md`
- `.context/system/{overview,conventions,risks}.md`

## 非目标

- Portal 截图、PipeWire 默认截屏、快捷键配置 UI、任意输入监听、设置 schema 改动、平台特定快捷键扩展或真实 GUI E2E。

## 预期文件

- `crates/pinora-app/Cargo.toml`：Linux target-only Portal 依赖声明。
- `crates/pinora-app/src/wayland_portal.rs`：Portal session、绑定、响应和信号 worker。
- `crates/pinora-app/src/hotkey.rs`：平台条件编译、Portal 动作与能力状态接线。
- `crates/pinora-app/src/desktop_shell.rs`：GUI 非阻塞轮询与诊断/tray 状态刷新。
- `crates/pinora-app/src/tray.rs`、`crates/pinora-app/src/tray_capabilities.rs`：既有能力标签刷新。
- `crates/pinora-app/src/lib.rs`：Linux 模块注册。
- `Cargo.lock`：锁定新增直接依赖。
- `AGENTS.md`、`.context/plans/100_wayland_global_shortcuts_portal.md`、`.context/system/{overview,conventions,risks}.md`：上下文与风险记录。

## 验收标准

1. 环境选择仅在 Linux + Wayland 尝试 Portal，其他路径保持既有原生/X11 或回退行为。
2. Portal session/bind/signal 逻辑在后台运行，固定信号标识可转换成既有截图动作；未知/取消/错误安全降级。
3. GUI 事件循环只轮询动作，不同步连接 session bus、等待 Portal `Request::Response` 或等待信号。
4. 设置主键更新会重建 Portal 绑定；失败时 tray/IPC 和既有已经成功的本地热键不受破坏。
5. 无新主窗口、控制窗口、事件循环或绕开 `window_policy` 的辅助窗口路径。

## 风险与回滚

- 风险：Portal 接口由桌面 backend 选择，当前环境成功不能推出其它 Wayland 会话支持；R-059 持续跟踪。
- 回滚：删除本任务新增适配器/依赖和中枢接线，纯 Wayland 返回 tray/IPC 降级，其他平台不变。

## 验证

- `cargo test -p pinora-app wayland_portal -- --nocapture`
- `cargo test -p pinora-app hotkey -- --nocapture`
- `cargo test -p pinora-app desktop_shell -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`
- `git diff --check`

## 完成记录

- 已完成 Linux target-only 的 XDG `GlobalShortcuts` Portal worker：session、BindShortcuts、Request 响应和 `Activated` 信号均在独立线程运行，GUI 线程只做非阻塞轮询。
- 已接入 `GlobalHotkeyHub`、Desktop Shell、tray 能力摘要和诊断状态；未知快捷键、取消、失联、缺失接口与重绑失败均保留 tray/IPC 回退，不创建控制窗口。
- 已修复非 Linux 条件编译分支，`cargo check --workspace --target x86_64-pc-windows-msvc` 通过。
- 已验证 Portal 4 项、Hotkey 16 项、Desktop Shell 39 项定向测试；`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace` 通过 app 298、core 90，2 项既有真实桌面测试忽略；格式、严格 Clippy、workspace check、`ctx validate` 与 `git diff --check` 均通过。
- 当前开发机的 session bus 未暴露 `org.freedesktop.portal.GlobalShortcuts`，运行时进入 `portal_interface_unavailable` 受控状态；真实 Wayland backend、授权 UI、热键触发、任务栏/Dock 和性能验证继续由 R-059 跟踪。
