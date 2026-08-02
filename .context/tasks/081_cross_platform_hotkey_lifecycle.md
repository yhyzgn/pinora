# 任务 081：跨平台全局热键生命周期

- 状态：已完成
- 计划：`.context/plans/081_cross_platform_hotkey_lifecycle.md`
- 规模：中
- 依赖：`crates/pinora-app/src/{hotkey.rs,desktop_shell.rs}` 既有 tray-only 事件循环与 `global-hotkey = 0.7`。
- 生产行为变更：Windows、macOS 与 Linux X11 将尝试注册既有全局热键；不支持或注册失败时显示受限能力并保留 tray/IPC。

## 任务目标

让 `GlobalHotkeyHub` 在 `winit` GUI 主线程创建、持有和释放 `GlobalHotKeyManager`，并从依赖提供的事件接收器在现有事件循环轮询动作，替代仅 Linux 使用的额外转发线程。

## 范围

- 重构 `hotkey.rs` 的 manager 生命周期、注册结果、事件轮询和测试注入边界。
- 保持 `desktop_shell` 只在既有 `about_to_wait` 调用 `poll_actions`，不创建 Pinora `winit` 热键窗口或额外事件循环；Windows 依赖后端的隐藏系统消息窗口不属于 Pinora 辅助窗口，真实任务栏隔离另行验证。
- 更新 `AGENTS.md` 工作指针、081 计划/任务、稳定事实和风险。

## 非目标

- 不实现 Portal、录制 UI、持久化热键配置、每绑定 tray 展示或系统权限设置入口。
- 不更改截图、贴图、窗口策略、导出、OCR、历史、设置 schema、IPC 帧和单实例行为。

## 预期文件

- `crates/pinora-app/src/hotkey.rs`
- `AGENTS.md`
- `.context/plans/081_cross_platform_hotkey_lifecycle.md`
- `.context/tasks/081_cross_platform_hotkey_lifecycle.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. Windows/macOS/Linux X11 都可编译同一 `GlobalHotkeyHub` 生命周期代码；Windows/macOS 不再返回 Linux 编译期开关导致的固定不可用。
2. `GlobalHotKeyManager` 由桌面 GUI 主线程创建和持有，所有动作仍由既有事件循环轮询；无新增线程、窗口或事件循环。
3. 必需热键注册失败、可选热键失败和无支持后端均受控降级，tray 与 IPC 入口不受影响；Wayland Portal 不被报告为已支持。
4. 单元测试、严格静态门禁、Windows target 编译、上下文校验和差异检查通过；macOS target 缺失时如实记录，交给 GitHub 原生 runner 复核。

## 验证

- `cargo test -p pinora-app hotkey -- --nocapture`
- `cargo test -p pinora-app tray_capabilities -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：`global-hotkey` 对 Linux 仅支持 X11，macOS/Windows 还依赖真实 GUI 主线程、用户权限和系统冲突状态。缓解：保留受控不可用状态、tray 和 IPC，后续真实平台探针单独覆盖。
- 风险：在静态全局事件接收器中残留测试事件。缓解：只在热键 manager 存活且 event state 为 Pressed 时映射动作，测试不共享真实全局 receiver。
- 回滚：恢复 Linux 专用 `spawn_global_hotkey_thread`；删除新 manager 字段即可，截图、IPC、tray 和窗口策略不变。

## 完成记录

- `GlobalHotkeyHub` 改为由 GUI 主线程直接持有 `GlobalHotKeyManager`，`poll_actions` 在既有 `about_to_wait` 中从依赖的静态事件接收器轮询；不再创建、转移或等待应用侧热键线程。
- F2/Ctrl+N 保持核心注册；Ctrl+Shift+S 和 F3 注册失败时只禁用对应可选入口。已加入“仅已注册且 Pressed 事件映射动作”和“不可用时保持 tray/IPC 降级”离线契约。
- 没有新增 Pinora `winit` 窗口、事件循环、权限请求、后台 worker、外部进程或网络路径；Windows 依赖后端可能创建隐藏 `WS_EX_TOOLWINDOW` 消息窗口，源码请求跳过任务栏但仍待实机验证。`window_policy` 守卫继续通过。
- 2026-08-02 验证通过：热键、tray 能力、窗口策略定向测试；格式、workspace 编译、严格 Clippy、离线全量测试（app 209 通过、2 项真实桌面测试忽略；core 85 通过）、Windows target 编译和差异检查。macOS target 本机未安装，GitHub 原生 runner 待复核。
