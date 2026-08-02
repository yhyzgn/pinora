# 计划 066：辅助窗口可见性策略收敛

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/066_auxiliary_window_visibility_policy.md`

## 目标

将 Pinora 的辅助窗口可见性入口收敛到 `window_policy`：Overlay、贴图、设置、历史和隐藏 display handle 都必须先通过受平台任务栏/Dock 策略约束的工厂创建；只有策略模块可将需要展示的窗口映射为可见并立即执行映射后隔离。空闲进程保持仅 tray 常驻，任何遮罩、编辑层、贴图或面板不得引入独立任务栏/Dock 项。

## 非目标

- 不新增窗口类型、托盘项目、系统菜单、窗口管理器脚本、截图、贴图交互、OCR 或平台依赖。
- 不把离线静态测试、CI 或 `--version` 启动描述为真实 Windows/macOS/X11/Wayland 任务栏、Dock 或合成器验收。

## 约束

- 所有生产窗口继续由现有 `window_policy` 工厂创建；display handle 必须永久保持隐藏，不能用来承载用户界面。
- 展示入口只能接收既有 `Overlay`、`Pin` 或 `Panel` 类型，映射后必须立即走已有 KWin 任务栏/分页器隔离；不新增窗口标题、脚本或平台协议。
- 隐藏和销毁路径保持原有资源释放、任务取消、贴图可见性快照和焦点语义；任务不改变 IPC、持久化、`PinId`、资产 generation、权限或状态字符串。

## 依赖关系

- 依赖 050 的 tray-only 桌面会话与托盘失败即退出契约。
- 依赖 054 的跨平台前置任务栏/Dock 属性与 KWin 映射后策略。
- 依赖 061 的单一 GUI 入口和建窗源码守卫。

## 阶段

1. 让辅助窗口工厂强制隐藏创建，新增唯一映射入口并在入口内执行映射后策略。
2. 迁移 Overlay、贴图、设置、历史和批量恢复路径，消除策略模块外的直接可见映射。
3. 扩展源码守卫为递归扫描，锁定事件循环、建窗和直接 `set_visible(true)` 的边界；执行全部门禁。

## 检查点

1. 任何窗口属性即使要求可见，工厂仍以隐藏状态创建；display handle 始终不映射。
2. 每个展示路径只经 `show_auxiliary_window`，并保留既有 KWin 映射后 `skipTaskbar`/`skipPager` 请求。
3. 无新增窗口、事件循环、平台菜单、后台任务或公共接口；空闲 tray-only 状态机不变。

## 计划级风险

- `set_visible(true)` 不会给标准 Wayland 提供通用任务栏协议；非 KDE Wayland 仍只能如实标记为待原生会话验证。
- 延后展示窗口可能暴露 Surface 初始化和首帧呈现的时序问题；定向测试可锁定路径，不能证明合成器实际映射时机。
- 现有历史、设置、Overlay 和贴图共享 `desktop_shell` 事件循环，错误迁移可能引入空白窗或遗漏恢复路径。

## 完成记录

- 辅助窗口工厂现强制隐藏创建；只有 `show_auxiliary_window` 可以映射 Overlay、贴图和面板，并在同一入口调用 KWin 映射后隔离。
- 已迁移截图 Overlay、贴图首次展示、延时恢复、贴图编辑恢复、历史和设置窗口；隐藏 display handle 不可经展示入口映射。
- 源码守卫递归扫描 `src/`，拒绝策略模块外创建事件循环/窗口、`with_visible(true)` 或 `set_visible(true)`；全量离线门禁通过，真实桌面窗口管理器验证仍开放。

## 变更前记录

```text
目的：把“仅 tray 常驻、所有临时窗口不进任务栏/Dock”从调用约定强化为单一可见性入口。
影响路径：window_policy、Overlay/贴图映射、历史与设置展示、源码守卫、上下文文档。
兼容性：不改变公共 IPC、持久化、PinId、资产 generation、权限或状态字符串。
外部副作用：仅现有窗口的创建/显示时序；无网络、外部进程、系统菜单或新窗口类型。
回滚点：恢复既有各窗口 `set_visible(true)` 调用并移除新入口；平台任务栏属性与 tray 生命周期保持独立。
验证场景：工厂隐藏创建、全部展示路径经策略入口、源码递归守卫、Overlay/贴图恢复、workspace 与上下文门禁。
```

## 完成标准

- 所有辅助窗口由工厂隐藏创建，所有显式显示路径都通过 `window_policy` 的唯一展示入口。
- 源码守卫递归拒绝策略模块外的事件循环、直接建窗、`with_visible(true)` 与 `set_visible(true)`。
- 既有 Overlay、贴图、历史、设置、延时恢复语义和 tray-only 状态机的定向测试通过。
- fmt、workspace check、严格 Clippy、全量离线测试、差异检查和 `ctx validate` 通过；真实桌面验证缺口明确保留。
