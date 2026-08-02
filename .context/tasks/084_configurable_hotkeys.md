# 任务 084：可配置主热键与无中断重绑

- 状态：已完成
- 计划：`.context/plans/084_configurable_hotkeys.md`
- 规模：大
- 依赖：081 的 GUI 线程 `GlobalHotkeyHub`、082 的 schema v2 设置与现有 `SettingsWindow`。
- 生产行为变更：用户可以在设置中录制区域截图与全屏截图的主键；保存时在旧键仍生效的情况下验证并切换 OS 注册。

## 任务目标

为 `AppSettings` 引入两个安全、可移植的主热键字段并迁移已有设置；让设置窗口可录制它们；让热键 hub 以可回滚的注册顺序应用更新。

## 范围

- `crates/pinora-core/src/{settings.rs,lib.rs}` 的热键领域模型、默认值、校验和测试。
- `crates/pinora-app/src/{settings_store.rs,settings_panel.rs,settings_window.rs,hotkey.rs,desktop_shell.rs,lib.rs}` 的 schema v3、录制、重绑和桌面接线。
- `AGENTS.md`、084 计划/任务、`.context/system/{overview.md,risks.md}`。

## 预期文件

- `crates/pinora-core/src/{settings.rs,lib.rs}`
- `crates/pinora-app/src/{settings_store.rs,settings_panel.rs,settings_window.rs,hotkey.rs,desktop_shell.rs}`
- `src/main.rs`
- `AGENTS.md`、`.context/plans/084_configurable_hotkeys.md`、`.context/tasks/084_configurable_hotkeys.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不实现 Portal、任意键盘钩子、平台专用复杂组合、辅助热键的编辑、主题重绘、诊断包、自动启动、截图策略或 OCR 设置扩展。
- 不以模拟注册、CI 或打包 smoke 验证真实桌面全局热键、任务栏/Dock、权限或冲突提示。

## 验收标准

1. v1/v2 读取迁移成可用的 v3 默认组合；v3 round-trip 保留字段；无效、重复和与固定备用键冲突的字段逐项修复而不破坏其他设置。
2. 设置窗口录制期间捕获物理键与当前修饰键，安全/受支持的组合才写入草稿；录制状态优先于窗口内截图快捷键。
3. `GlobalHotkeyHub` 的新组合完整预注册后才撤销旧组合；任何失败不修改当前已工作的动作映射。成功保存后 runtime 和 tray 能力摘要保持可用，且不创建新窗口或事件循环。
4. 固定 `Ctrl+N`、`Ctrl+Shift+S` 区域备用键、`pinora capture` IPC 和 tray 操作保持可用。

## 验证

- `cargo test -p pinora-core settings -- --nocapture`
- `cargo test -p pinora-app settings_store -- --nocapture`
- `cargo test -p pinora-app settings_panel -- --nocapture`
- `cargo test -p pinora-app hotkey -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：OS 拒绝新组合或撤销失败造成无键或重复键。缓解：先验证并预注册、失败回滚、只在成功后保存；真实平台仍需探针。
- 风险：设置录制的快捷键和截图快捷键竞争。缓解：录制状态优先、物理键白名单和纯逻辑映射测试。
- 风险：schema v3 破坏已有设置。缓解：保留 v1/v2 严格解码、固定长度记录和读回校验。
- 回滚：删除 v3 字段、录制与 rebind 调用，恢复固定 F2/F3；既有 v1/v2 文件与截图/贴图资产不受影响。

## 完成记录

- 完成时间：2026-08-02。
- 交付：schema v3 保存区域/全屏主热键；v1/v2 自动迁移为 F2/F3。录制只接受 F1-F12 或带 Ctrl/Alt/Super 的字母物理键，拒绝裸字母、主键重复及 Ctrl+N/Ctrl+Shift+S 冲突。
- 交付：设置窗口的录制状态优先处理键盘输入，Esc 取消，不触发截图。热键 hub 用可注入注册器事务先注册新键、失败清理新键、成功后再移除旧键；OS 后端受限时设置仍可保存，tray/IPC 不受影响。
- 验证：core 设置 5 项、settings store 9 项、settings panel 7 项、hotkey 14 项、Overlay 回归 30 项；workspace 离线测试应用 227 项、core 88 项通过，2 项真实桌面测试跳过；`cargo fmt --check`、workspace check、严格 Clippy、Windows target、`git diff --check` 通过。
- 未覆盖风险：未执行真实桌面全局键、冲突、焦点、休眠恢复、任务栏/Dock/分页器或 Wayland Portal 探针；这些仍在 R-045 中跟踪。
