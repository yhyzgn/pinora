# 任务 086：tray 热键标签同步

- 状态：已完成
- 计划：`.context/plans/086_tray_hotkey_labels.md`
- 规模：小
- 依赖：084 的可配置主热键及其可回滚保存顺序。
- 生产行为变更：tray 的区域/全屏截图菜单显示当前已成功保存的主热键组合。

## 任务目标

将 `AppTray` 的两个截图菜单项变成可原地更新的句柄，并仅从设置成功分支把持久化绑定同步为菜单文本。

## 范围

- `crates/pinora-app/src/{tray.rs,desktop_shell.rs}` 的初始标签、更新 API、成功提交接线和定向测试。
- `AGENTS.md`、086 计划/任务、`.context/system/{overview.md,risks.md}`。

## 预期文件

- `crates/pinora-app/src/{tray.rs,desktop_shell.rs}`
- `AGENTS.md`
- `.context/plans/086_tray_hotkey_labels.md`
- `.context/tasks/086_tray_hotkey_labels.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不重写热键、设置、诊断、tray 初始化、平台后端或窗口策略；不新增依赖。
- 不把菜单标签作为全局注册状态、权限状态或 Portal 支持的声明。

## 验收标准

1. 默认和自定义 `HotkeyBinding` 均生成稳定、短、无控制字符的区域/全屏标签。
2. `AppTray` 能保存并更新两个菜单项文本；设置成功才调用同步，任何失败路径保留现有文本。
3. `TrayAction`、菜单 ID、区域备用键、IPC、热键 hub、诊断实际状态和 tray-only 约束不变。

## 验证

- `cargo test -p pinora-app tray -- --nocapture`
- `cargo test -p pinora-app hotkey -- --nocapture`
- `cargo test -p pinora-app desktop_shell -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：标签在设置写入失败时提前变化，造成 UI 与运行时不一致。缓解：更新只位于成功落盘且 runtime 应用后的分支；重绑失败更早返回。
- 风险：标签将受限配置误报为 OS 注册成功。缓解：能力摘要/诊断继续使用实际 `GlobalHotkeyHub` 状态，文案只表示当前配置。
- 风险：原生 tray 后端不即时刷新。缓解：不重建 tray 或新建窗口；保留真实桌面验证缺口。
- 回滚：删除更新 API 与成功分支调用，恢复固定 F2/F3 文案；现有热键、设置、tray 和 IPC 不受影响。

## 完成记录

- 完成时间：2026-08-02。
- 交付：tray 在创建时以当前区域/全屏 `HotkeyBinding` 生成菜单文字，并持有两个菜单项以原地更新；格式使用受限 `Display` 输出，定向测试证明默认/自定义组合短且无控制字符。
- 交付：标签同步位于设置成功落盘并应用 runtime 的分支；热键重绑失败或设置保存失败不会执行同步，因此保持旧标签、旧映射和既有受控失败状态。
- 验证：`cargo test -p pinora-app tray -- --nocapture`（15 项）、`hotkey`（16 项）、`desktop_shell`（30 项）、`cargo fmt --check`、workspace check、严格 Clippy、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（应用 233 项、核心 88 项通过，2 项真实桌面测试跳过）、Windows target、`git diff --check`、ctx validate 均通过。
- 已知风险：没有原生 tray 会话证据，不能把本地菜单标签或 GitHub CI 当作实际系统热键注册、tray 重绘、任务栏/Dock/分页器或焦点行为验收。
