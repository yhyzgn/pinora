# 计划 084：可配置主热键与无中断重绑

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/084_configurable_hotkeys.md`

## 目标

把区域截图和单显示器全屏截图的主全局热键从固定常量升级为版本化本地设置。用户可在现有设置窗口录制受支持的组合；保存时必须先成功注册全部新组合，再移除旧组合，冲突、无效组合或持久化失败不能造成热键空档。

## 非目标

- 不实现 Wayland XDG GlobalShortcuts Portal、系统级快捷键设置跳转、后台键盘监听、录制窗口、热键宏、热键导入导出或任意外部命令执行。
- 不改变 `Ctrl+N` 与 `Ctrl+Shift+S` 既有区域截图备用键、IPC 协议、截图、贴图、OCR、历史、导出、任务栏/Dock 或 tray-only 语义。
- 不以 CI 或 target 编译声称任意平台的真实全局热键、冲突提示或窗口管理器行为已经验收。

## 依赖关系

- 依赖 081 已在 GUI 主线程持有 `GlobalHotKeyManager`，以及 082 的版本化设置迁移和原子保存。
- 不新增第三方依赖；热键平台适配继续使用既有 `global-hotkey = 0.7`。

## 约束

- 设置领域只保存有限、跨平台可映射的物理键和修饰键，不保存平台 SDK 字符串；裸字母/数字必须被拒绝，单独功能键可以使用。
- 区域和全屏主键必须不同，也不得占用固定区域备用键；设置解码对无效字段逐项恢复默认值。
- 配置更新先预注册新键；任一注册、撤销或设置原子保存失败时恢复/保留旧运行时注册，不发布“已保存”状态。
- 热键 manager 始终在现有 GUI 线程创建、轮询和销毁；不新增窗口、事件循环、工作线程、外部进程、网络或权限绕过。

## 阶段

1. 在 core 定义稳定热键组合、默认值和逐项校验，设置 codec 升级至 schema v3 并迁移 v1/v2。
2. 在设置窗口增加两个主键的录制状态和受支持物理按键映射；录制优先于本窗口的截图快捷键。
3. 在 `GlobalHotkeyHub` 实现预注册、回滚和事件映射更新；只在 rebind 成功后持久化并应用 runtime 设置。
4. 覆盖 schema 迁移、重复/无效组合、录制映射、重绑成功/失败契约和既有 tray-only 守卫，执行跨平台静态门禁。

## 检查点

1. v1/v2 设置无损迁移为默认 F2/F3；v3 往返保持两个配置组合；损坏组合仅修复该字段。
2. 设置窗口录制时 F2/F3 不触发截图；录到相同/不支持/不安全组合会停留在设置窗口并显示稳定失败状态。
3. 新键冲突时旧键仍可用；新键注册与配置写入成功后才释放旧键；`Ctrl+N`/`Ctrl+Shift+S` 备用区域键保持不变。

## 计划级风险

- OS 热键 API 不能提供跨平台的原子多键事务；实现只能采用预注册、补偿撤销和离线注入测试，仍需原生会话验证冲突与回滚。
- 纯 Wayland 没有 Portal adapter 时只能持久化组合并保留 tray/IPC，不能把录制成功写成全局注册成功。

## 完成标准

- 领域、codec、设置面板和热键映射均有离线测试；运行时更新不创建 Pinora 窗口、事件循环或后台线程。
- 定向、workspace、Windows target、严格 Clippy、差异和 ctx 门禁通过；GitHub 三原生 runner 复核编译。
- 真实 Windows/macOS/X11/Wayland 的录制、冲突、睡眠恢复、任务栏/Dock 和 Portal 行为如实保留为未覆盖风险。

## 变更前记录

```text
目的：将高频截图主键从固定实现升级为可持久化、可录制且无中断重绑的设置。
影响路径：core 设置模型、settings codec/面板/窗口事件、hotkey hub、desktop shell、上下文和风险。
兼容性：保留既有区域备用键、IPC、截图动作、数据文件迁移、tray-only 和权限语义；v1/v2 自动迁移。
外部副作用：成功保存时对本机 OS 重新注册两个主热键；失败只保留现有注册，不联网、不下载、不创建窗口。
回滚点：移除 v3 热键字段和 rebind 分支，迁移后设置仍可按 v2 解析；恢复固定 F2/F3 与现有备用键。
验证场景：迁移、组合校验、录制优先级、重绑成功/失败、保存失败、事件分发、窗口策略和质量门禁。
```

## 完成记录

- `pinora-core` 新增受限 `HotkeyCode`、`HotkeyModifiers` 和 `HotkeyBinding` 领域模型；裸字母被拒绝，功能键可独立使用，区域与全屏主键不能重复或占用 Ctrl+N/Ctrl+Shift+S 兼容入口。
- 设置 schema 从 v2 的 19 字节升级至 v3 的 23 字节；v1/v2 在读取时保留已有字段并使用默认 F2/F3 主键迁移。v3 无效组合逐字段恢复默认，不影响另一主键或其他设置。
- 设置窗口以既有辅助窗口录制物理键；录制优先于 F2/F3 本地截图快捷键，不创建录制窗口。`GlobalHotkeyHub` 在 GUI 线程预注册全部新键、失败回滚新键并保留旧映射；设置落盘失败后尝试恢复旧绑定。受限后端可保存组合但不会报告已注册。
- 已验证：`cargo fmt --check`、`cargo check --workspace`、`cargo test -p pinora-core settings -- --nocapture`（5 项）、`cargo test -p pinora-app settings_store -- --nocapture`（9 项）、`cargo test -p pinora-app settings_panel -- --nocapture`（7 项）、`cargo test -p pinora-app hotkey -- --nocapture`（14 项）、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`（30 项）、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（应用 227 项、核心 88 项通过，2 项真实桌面测试跳过）、严格 Clippy、Windows target、差异检查通过。
- 未覆盖风险：真实 Windows/macOS/X11 的组合录制、系统冲突、权限、睡眠恢复、原生状态可见性与任务栏/Dock/分页器隔离，以及 Wayland Portal 仍需隔离桌面探针；这些风险没有被静态门禁或 fake registrar 视为已解决。
