# 计划 079：托盘能力摘要

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/079_tray_capability_summary.md`

## 目标

在不创建诊断窗口、系统通知、外部探针或后台任务的前提下，利用现有 tray 菜单展示本次启动的截图、全局热键、系统图像剪贴板与本地 OCR 可用性。能力文字必须是有限且脱敏的摘要；全局热键以 `GlobalHotkeyHub` 的实际注册结果为准，不能沿用启动 probe 在 Linux 上的乐观猜测。

## 非目标

- 不实现诊断窗口、诊断包导出、系统权限跳转、日志查看、能力热刷新、外部进程探测、网络请求或遥测。
- 不改变 `CapabilitySnapshot` 的持久化形状、截图/热键/OCR/剪贴板协议、错误码、设置、历史、任务监督或窗口策略。
- 不把 tray 菜单文字、离线测试或 GitHub CI 作为真实系统 tray、权限、任务栏/Dock/分页器的证据。

## 依赖关系

- 依赖 044 的 tray 菜单动作、050/061/066 的 tray-only 与辅助窗口展示边界，以及 078 的受控 tray 反馈模型。
- 复用 `AppRuntime` bootstrap 产生的 `CapabilitySnapshot`、`GlobalHotkeyHub` 的实际注册状态与 `tesseract_available` 的无进程路径检查；不改变它们的领域/平台契约。

## 约束

- 所有菜单项继续属于已创建的 `AppTray`，均为禁用的只读项；不得创建 `Window`、`EventLoop`、worker、系统通知或原生系统菜单窗口。
- 文案只由受限布尔快照和固定中文映射生成，不使用 `CapabilitySnapshot.notes`、平台错误原文、路径、显示器、OCR 文本、剪贴板内容或环境变量。
- OCR 状态只检查已有的本地可执行文件路径，不启动 `tesseract` 或枚举模型；截图和剪贴板状态只复用运行时已探测的布尔值。
- tray-only 边界不变：空闲进程只驻留 tray；Overlay、贴图、历史、设置和任何辅助层仍须隐藏创建并经唯一展示入口显示。

## 阶段

1. 建立纯 tray 能力摘要模型，覆盖有限文案、隐私边界和实际热键结果覆盖，并补充单元测试。
2. 将摘要作为现有 `AppTray` 的禁用菜单项创建，保持截图、延时、窗口捕获、贴图、设置、历史和退出动作顺序/可用性不变。
3. 在桌面壳启动 tray 前组装当前快照；热键使用 `GlobalHotkeyHub::status().available`，OCR 使用无进程的已有 `tesseract_available` 检查。
4. 运行定向、workspace、上下文门禁，并记录真实 tray/权限/窗口管理器验收缺口。

## 检查点

1. 用户从 tray 可扫描本次启动的截图、全局热键、系统图像剪贴板与本地 OCR 是否可用或受限，且不会看到原始后端/路径/内容。
2. 热键菜单状态与 `GlobalHotkeyHub` 的实际注册结果一致；失败不阻止 tray、IPC 或手动截图菜单继续工作。
3. 生产源码未新增窗口、事件循环、worker、外部进程、系统通知或网络路径；现有窗口策略源码守卫持续通过。

## 计划级风险

- Linux StatusNotifier/AppIndicator、Windows 通知区域或 macOS 菜单栏可能折叠、延迟或忽略禁用条目；离线测试不能证明其原生可见性。
- 启动时能力只是一份快照，用户运行期间安装/移除剪贴板或 OCR 依赖不会立即刷新；本任务不伪造热刷新。
- `global-hotkey` 在部分 Wayland 会话不能稳定注册；菜单必须如实显示受限，并保留 tray/IPC 手动入口。

## 变更前记录

```text
目的：在 tray-only 模型内给出受控的启动能力摘要，避免用户把不可用功能误认为已可用。
影响路径：纯能力摘要模型、既有 tray 菜单构造、desktop_shell 启动装配、测试与上下文。
兼容性：不改变公共接口、持久化、状态字符串、任务 owner/generation、租户或权限语义。
外部副作用：无新窗口、事件循环、worker、外部进程、系统通知、网络或真实共享基础设施访问。
回滚点：移除摘要模型与禁用菜单项，并恢复原有 `AppTray::try_new` 调用；截图、热键、OCR、剪贴板和窗口策略不受影响。
验证场景：文案脱敏、实际热键覆盖、菜单动作保持、tray-only 源码守卫、workspace 与上下文门禁。
```

## 完成标准

- 现有 tray 菜单展示受限、脱敏且与实际热键注册结果一致的能力摘要，手动操作入口保持可用。
- 不新增窗口、事件循环、worker、外部进程、通知或网络副作用；辅助窗口继续满足 taskbar/Dock/分页器隔离源码守卫。
- 定向测试、fmt、workspace check、严格 Clippy、全量离线测试、差异检查与 `ctx validate` 通过；原生 tray/权限验收缺口如实记录。

## 完成记录

- 新增 `TrayCapabilitySummary` 纯模型，固定输出截图、全局热键、系统图像剪贴板和
  本地 OCR 的可用/受限中文标签；它不读取或显示 runtime notes、路径、后端错误、
  OCR 文本或剪贴板内容。
- 既有 tray 在最近操作状态下附加禁用的“环境能力（本次启动）”标题和四条只读项；
  所有截图、延时、窗口捕获、贴图、设置、历史和退出动作保持原有行为。
- 桌面壳以 `GlobalHotkeyHub::status().available` 覆盖 bootstrap probe 的热键猜测；
  OCR 仅使用已有 `tesseract_available` 路径检查，不启动 CLI、worker 或新事件循环。
- 2026-08-02 已通过能力摘要、tray、桌面状态机和窗口策略定向测试，
  `cargo fmt --check`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 206 项中 204 通过、
  2 项真实桌面测试忽略；core 85 通过）、`git diff --check` 与 `ctx validate`。
- 未验证项：真实 Linux/Windows/macOS tray 对禁用能力菜单项的可见性、辅助功能语义、
  权限实际状态和任务栏/Dock/分页器行为，仍须在原生桌面会话验收。
