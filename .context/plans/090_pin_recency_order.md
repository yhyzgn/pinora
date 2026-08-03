# 计划 090：贴图最近使用排序

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/090_pin_recency_order.md`

## 目标

使已交付的 tray 贴图列表按“最近使用”排序，优先呈现最近获得焦点或由 tray 唤起的既有贴图。排序仅保存在 `DesktopApp`/`PinWin` 的进程内 UI 状态中，不影响 `PinId`、领域状态、窗口创建策略或持久化格式。

## 非目标

- 不新增窗口、线程、事件循环、截图、OCR、导出、持久化设置、历史记录或依赖。
- 不使用标题、图像内容、OCR、路径、坐标或内部 ID 作为用户可见标签。
- 不改变贴图的创建、关闭、锁定、缩放、透明度、置顶或可见性语义。
- 不将离线焦点事件、target 编译或单元测试称为真实跨平台焦点、tray 刷新或任务栏/Dock/分页器行为证据。

## 依赖关系

- 依赖 089 的 `TrayPinListEntry`、动态子菜单和 `ActivatePin(PinId)` 映射。
- 依赖 `desktop_shell` 已有的 `PinWin` UI ownership 和 `WindowEvent::Focused(true)`。
- 依赖既有 `window_policy::show_auxiliary_window`，保持 tray 唤起不会绕过窗口隔离。

## 约束

- 最近使用时间戳仅为进程内饱和计数；进程重启后重置，绝不写入领域、设置、历史或日志。
- 新创建、获得焦点、tray 单贴图唤起均更新计数；排序按 recency 降序，值相同才按 `PinId` 做确定性回退。
- 仅重建既有 tray 子菜单；焦点事件和唤起不得创建窗口、改变窗口层级或发起后台工作。
- 托盘标签继续仅含顺序号和可见性；内部 recency 与 PinId 不得输出至菜单或日志。

## 阶段

1. 扩展无敏感的 tray 列表快照，使其可按 recency 降序并用 PinId 稳定打破并列。
2. 为 `PinWin` 增加内存 recency，并在创建、`Focused(true)` 和 `ActivatePin` 路径更新，随后刷新现有 tray 列表。
3. 覆盖排序和焦点更新的纯契约，运行定向、workspace、跨 target、严格静态、差异和上下文门禁。

## 检查点

1. 两个不同 recency 的贴图按最新在前；相同 recency 的排序与 HashMap 迭代无关。
2. 新建和 tray 唤起后的贴图在下一次子菜单刷新中排在前面；真实焦点事件仅更新同一既有贴图。
3. 计数饱和不 panic、不回绕，不影响身份或其他贴图行为。
4. 不新增展示入口、可见窗口、后台资源或敏感菜单内容；原有 `window_policy` 守卫继续通过。

## 计划级风险

- 各平台可能不产生或延迟产生 winit 焦点事件，且 OS 可能拒绝聚焦请求；列表顺序因此可能与用户感知不完全一致，需原生会话验收。
- 高频焦点切换会触发子菜单刷新；受既有 Pin 数量上限限制，且仅在焦点变化时执行，但真实 tray 后端流畅度仍需验证。

## 变更前记录

```text
目的：补齐多贴图“按最近使用排序”能力，使 tray 列表优先显示用户刚操作或唤起的贴图。
影响路径：TrayPinListEntry 排序、PinWin 进程内 UI 字段、DesktopApp 焦点/唤起路径、上下文与风险记录。
兼容性：不改公共接口、持久化数据、PinId、状态字符串、截图/OCR/导出结果、权限或租户语义。
外部副作用：焦点变化时原地更新 tray 子菜单；不联网、不写磁盘、不创建窗口、不请求权限。
回滚点：删除 recency 字段、排序键和焦点更新，恢复 089 的 PinId 稳定排序。
验证场景：新建、tray 唤起、焦点、相同 recency、饱和计数、无内容标签、窗口策略和 workspace 门禁。
```

## 完成标准

- tray 贴图列表按最近使用排序，且排序稳定、无敏感信息、仅属进程内 UI 状态。
- 焦点和 tray 唤起只操作既有窗口并复用既有窗口策略。
- 定向、workspace、跨 target、严格静态、差异和 `ctx validate` 通过；真实平台焦点/tray 行为记录为风险。

## 完成记录

- `TrayPinListEntry` 增加仅内部使用的 `last_used`，排序固定为 recency 降序、PinId 升序；列表标签不显示两个值。
- `DesktopApp` 持有进程内饱和时钟，`PinWin` 仅持有对应内存序号；新建、`Focused(true)` 与 tray 唤起推进序号并刷新既有子菜单，未知窗口不会推进时钟。
- 既有展示、焦点、重绘和关闭路径不变；本任务未新增窗口、线程、截图、持久化、依赖或领域状态。
- 已验证：`cargo fmt --check`、tray/desktop shell/window policy 定向测试、`cargo check --workspace`、严格 Clippy、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 251 通过、2 忽略；core 88 通过）、Windows target 编译、`git diff --check` 与 `ctx validate` 均通过。
