# 计划 100：Wayland GlobalShortcuts Portal

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/100_wayland_global_shortcuts_portal.md`

## 目标

为 Linux 纯 Wayland 会话接入运行时探测的 XDG `GlobalShortcuts` Portal，使已保存的区域与全屏截图主键可在 Portal 后端实际授权并绑定时触发既有截图动作，同时保持 tray-only 生命周期和 X11/IPC 回退。

## 非目标

- 不改造截图后端，不把 Portal/PipeWire 截图作为默认高性能路径。
- 不实现任意键盘监听、后台输入钩子、系统设置写入、快捷键编辑 UI、Portal 截图或新窗口。
- 不替换 Windows/macOS/Linux X11 的 `global-hotkey` 后端，不修改设置持久化格式或对外 IPC 协议。
- 不将离线 mock、D-Bus 接口编译、CI 或当前开发机的 Portal 响应宣称为其它桌面环境的原生验收。

## 约束

- Portal 只在 Linux + 运行时 Wayland 会话尝试；X11 优先保留现有 `global-hotkey` 逻辑。
- 所有 D-Bus 请求、权限/绑定响应和信号等待必须离开 `winit` GUI 事件循环；GUI 线程仅非阻塞读取受限动作队列。
- 仅使用空 `parent_window`，不为 Portal 创建控制窗口；Overlay、贴图、设置、历史和诊断窗口不得因本任务出现在任务栏、Dock 或分页器。
- 仅请求固定的 `capture-region` 与 `capture-full-display` 标识；Portal 返回未知标识、取消、缺失后端、失联或版本不足必须受控降级，tray 和 `pinora capture` IPC 不受影响。
- 引入依赖必须限定 Linux target，使用已有锁文件中的 `zbus` 版本和最小功能面；不得新增大型桌面框架。

## 依赖关系

- 依赖 `GlobalHotkeyHub`、已有安全 `HotkeyBinding`、`DesktopApp::poll_external_actions`、诊断/托盘能力摘要和现有 tray-only 窗口策略。
- 接口依据 XDG Desktop Portal `GlobalShortcuts` v2、`Request`/`Session` 约定及 Freedesktop shortcuts 触发器规范。

## 阶段

1. 建立 Linux target-only Portal 适配器、运行时会话门槛和纯数据映射/响应解析测试。
2. 在后台线程创建 session、绑定固定动作、订阅 `Activated`，把已知标识转发到 GUI 线程队列。
3. 将可用性、重绑和失败降级接入 `GlobalHotkeyHub`、托盘/诊断状态，执行 workspace 与上下文门禁。

## 检查点

1. Wayland/X11/非 Linux 的后端选择互斥，Portal 触发器与标识映射有离线测试。
2. Portal 请求、取消、未知信号、后台失联和重绑均不阻塞 GUI 事件循环且不丢失 tray/IPC 回退。
3. 定向测试、`cargo fmt --check`、workspace check、严格 Clippy、workspace 测试、`ctx validate` 和 `git diff --check` 通过。

## 计划级风险

- 各 Wayland desktop/backend 对 `GlobalShortcuts` Portal 的实现和用户授权 UI 不一致；真实 KDE Wayland、GNOME Wayland 等原生会话仍需单独验收，记录在 R-059。
- Portal 绑定可能等待用户交互；后台初始化不能阻塞 tray、事件循环或已有截图路径。

## 变更前记录

```text
目的：让具备 XDG GlobalShortcuts Portal 的纯 Wayland 会话可将两项已保存截图主键路由到既有动作。
影响路径：crates/pinora-app/Cargo.toml、hotkey.rs、wayland_portal.rs、desktop_shell.rs、lib.rs；.context 计划、任务、system 与 risks。
兼容性：不改变设置 schema、IPC、公开业务状态、截图数据、租户或权限语义；Linux Wayland 的热键能力从固定降级扩展为运行时可用。
外部副作用：Portal 可显示系统提供的快捷键授权/配置 UI；不联网、不创建控制窗口、不访问真实共享基础设施。
回滚点：移除 Portal 适配器及 Linux target 依赖，恢复纯 Wayland 的 tray/IPC 降级；X11/Windows/macOS 后端不变。
验证场景：环境门槛、固定标识、触发器编码、后台事件到动作队列、未知/取消/失联、X11 优先、重绑降级、workspace 门禁。
```

## 验收标准

1. Linux 纯 Wayland 上仅在 Portal 接口可用且绑定成功后报告 Portal 全局热键可用；X11、非 Linux、缺失/拒绝/取消/失联 Portal 均保留既有受控行为。
2. `Activated` 只将固定 `capture-region`、`capture-full-display` 映射为既有 `ActionId`；未知、重复及 `Deactivated` 不会触发截图。
3. session、绑定、信号等待和销毁均在后台执行；GUI 线程不执行阻塞 D-Bus 调用，空闲态不创建任何主窗口或 Portal 控制窗口。
4. 设置重绑后 Portal 使用最新两项主键；失败不破坏当前已可用的本地后端、tray 或 IPC 入口。
5. 所有验收标准有定向离线契约和静态门禁证据；原生 Wayland 后端/授权/任务栏/Dock/性能差异明确保留为开放风险。

## 完成标准

- 代码与上下文满足全部验收标准，并把真实桌面验证边界与回滚路径记录清楚。

## 风险与回滚

- 风险：Portal 后端未实现、实现版本不足或拒绝绑定。缓解：运行时探测、固定受控状态、后台隔离与 tray/IPC 回退。
- 风险：系统授权 UI 等待或 D-Bus 失联影响常驻体验。缓解：后台线程、GUI 非阻塞队列、有限状态更新和不可用降级。
- 回滚：删除 Portal 适配器和 Linux target 依赖；纯 Wayland 恢复 tray/IPC 降级，不触及设置、截图、贴图或窗口策略。

## 完成记录

- 已完成 Linux target-only 的 XDG `GlobalShortcuts` Portal worker：session、BindShortcuts、Request 响应和 `Activated` 信号均在独立线程运行，GUI 线程只做非阻塞轮询。
- 已接入 `GlobalHotkeyHub`、Desktop Shell、tray 能力摘要和诊断状态；未知快捷键、取消、失联、缺失接口与重绑失败均保留 tray/IPC 回退，不创建控制窗口。
- 已修复非 Linux 条件编译分支，`cargo check --workspace --target x86_64-pc-windows-msvc` 通过。
- 已验证 Portal 4 项、Hotkey 16 项、Desktop Shell 39 项定向测试；`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace` 通过 app 298、core 90，2 项既有真实桌面测试忽略；格式、严格 Clippy、workspace check、`ctx validate` 与 `git diff --check` 均通过。
- 当前开发机的 session bus 未暴露 `org.freedesktop.portal.GlobalShortcuts`，运行时进入 `portal_interface_unavailable` 受控状态；真实 Wayland backend、授权 UI、热键触发、任务栏/Dock 和性能验证继续由 R-059 跟踪。
