# 计划 081：跨平台全局热键生命周期

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/081_cross_platform_hotkey_lifecycle.md`

## 目标

将既有 `global-hotkey` 的生命周期收敛到桌面 GUI 主线程，使 Windows、macOS 和 Linux X11 能按该依赖的运行约束尝试注册固定 P0 热键。能力状态必须取自实际注册结果；Linux Wayland 没有 XDG GlobalShortcuts Portal 实现时必须保持受限和可见降级。

## 非目标

- 不实现热键录制、配置持久化、冲突建议、热更新或 Wayland Portal；这些需要后续设置与平台适配任务。
- 不改变固定动作集合、IPC 协议、托盘菜单、窗口创建策略、截图、导出、OCR、历史或持久化数据形状。
- 不新增依赖、Pinora `winit` 辅助窗口或事件循环，不连接任何真实共享服务，也不把 target 编译或 CI 当作真实桌面热键验收。Windows 依赖后端可能内部创建隐藏的 `WS_EX_TOOLWINDOW` 消息窗口；其是否始终不进入任务栏仍由真实桌面探针确认。

## 依赖关系

- 依赖现有 `global-hotkey = 0.7`、`winit` 主事件循环、`GlobalHotkeyHub`、tray 能力摘要和单实例 IPC；不新增第三方依赖。
- 保留 050/054/066 的 tray-only 窗口策略、079 的实际热键摘要和 080 的既有已完成导出路径。
- 后续热键录制、配置持久化、冲突建议、Wayland Portal 及真实平台探针必须拆为独立任务，不以本任务的编译结果替代。

## 约束

- `GlobalHotKeyManager` 必须在 GUI 事件循环线程创建、持有和销毁；禁止移动到后台线程或创建独立事件循环。
- Linux 仅声明 X11 后端，纯 Wayland 必须保留 tray 和 `pinora capture` IPC 降级，禁止暗示已支持 XDG GlobalShortcuts Portal。
- 空闲态只保留 tray、已成功注册的热键、IPC 和帧缓存；禁止新增 Pinora `winit` 窗口、后台键盘监听、外部进程、网络访问或权限绕过。Windows 依赖内部消息窗口必须保持隐藏且不进入任务栏。
- 未注册、未知或 Released 的热键事件不能触发截图动作；注册失败不能影响 tray、IPC 和既有窗口策略。

## 阶段

1. 审计 `global-hotkey 0.7.0` 的平台线程约束并冻结注册、轮询和析构的离线契约。
2. 移除应用层仅 Linux 的工作线程门控：在 GUI 主线程创建并持有 manager，在现有 `about_to_wait` 轮询静态事件接收器。
3. 更新 Linux/Windows/macOS 的状态说明和风险登记，执行 workspace 门禁与 Windows target 编译；macOS 由原生 GitHub runner 复核。

## 检查点

1. Windows/macOS 构建不再走“当前 build 不启用热键”的硬编码失败分支。
2. manager 的创建、注册、事件轮询和销毁都发生在桌面事件循环所属线程；不新增后台线程或事件循环。
3. 全局热键不可用时 tray 与 IPC 仍保持可用，Wayland Portal 不被错误声明为已支持。

## 完成标准

- Windows/macOS/Linux X11 共享同一 manager 生命周期实现，Windows/macOS 不再由应用层编译期开关直接禁用。
- F2/Ctrl+N 的核心注册、Ctrl+Shift+S/F3 的可选注册、仅 Pressed 事件分发和受控不可用降级均有离线契约。
- workspace 门禁、Windows target 编译、上下文校验和差异检查通过；macOS 的真实编译与桌面行为由原生 runner 或实机证据单独验证并如实记录。

## 计划级风险

- `global-hotkey` 仅支持 Linux X11；纯 Wayland 仍需要 Portal 或系统快捷方式作为后续独立任务。
- Windows/macOS 的真实权限、冲突、睡眠恢复与热键触发需要在原生桌面会话验证；本任务只建立正确生命周期与编译契约。

## 变更前记录

```text
目的：修复跨平台构建中人为禁用的全局热键，并按依赖的 GUI 线程约束管理其生命周期。
影响路径：hotkey 适配器、桌面 shell 热键轮询、测试、上下文与工作指针。
兼容性：不改变既有固定动作、IPC、数据、状态字符串、租户或权限语义。
外部副作用：应用启动时仍只尝试既有 OS 热键注册；不新增权限请求、后台进程、窗口、网络或共享服务。
回滚点：恢复 Linux 专用热键启动分支；IPC 和 tray 手动入口保持不变。
验证场景：注册成功、必需注册失败、可选绑定失败、事件去抖、析构、Windows target 编译和 workspace 回归。
```

## 完成记录

- `GlobalHotkeyHub` 现在直接持有 `GlobalHotKeyManager`，由 `run_desktop_shell` 在创建 `winit` 主事件循环后、进入循环前构造，并在同一 `DesktopApp` 实例中轮询事件和析构；已删除仅 Linux 使用的应用侧热键线程。Pinora 没有新增 `winit` 辅助窗口；Windows 依赖后端的隐藏 `WS_EX_TOOLWINDOW` 消息窗口仍需实机确认不会显示在任务栏。
- F2 与 Ctrl+N 注册失败会整体回退；Ctrl+Shift+S 与 F3 保持可选，状态说明逐项反映注册结果。仅已注册的 `Pressed` 事件可以触发动作，未知或 `Released` 事件被拒绝。
- 已根据 `global-hotkey 0.7.0` 文档保留平台边界：Windows 使用 GUI 事件循环线程，macOS 使用主 GUI 线程，Linux 仅 X11；纯 Wayland Portal 未实现，tray 与 `pinora capture` IPC 始终可降级。
- 2026-08-02 已通过定向热键、tray 能力和窗口策略测试，`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 209 通过、2 项真实桌面测试忽略；core 85 通过）、Windows target 编译和差异检查。macOS 交叉 target 未安装，后续由 GitHub 原生 macOS runner 复核。
