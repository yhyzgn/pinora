# 任务 044：托盘菜单动作闭环

- 状态：已完成
- 计划：`.context/plans/044_tray_actions.md`
- 规模：中
- 依赖：`.context/tasks/041_settings_panel.md`、`.context/tasks/042_history_browser.md`、`.context/tasks/043_history_management.md`
- 生产行为变更：是；新增托盘控制入口。

## 范围

- 扩展 `TrayAction` 和 `AppTray` 菜单项 ID 映射。
- 在 `desktop_shell` 接入设置、历史、显示/隐藏/关闭全部贴图动作。
- 保持 tray 创建失败时的现有手动入口与错误提示。

## 任务目标

让无键盘焦点的桌面用户也能从托盘完成主要窗口和贴图管理操作。

## 非目标

- 不实现贴图列表动态子菜单、全屏截图、诊断包和原生可访问性。

## 预期文件

- `crates/pinora-app/src/tray.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `AGENTS.md`、`.context/plans/044_tray_actions.md`、`.context/tasks/044_tray_actions.md`
- `.context/system/overview.md`、`.context/system/risks.md`

## 验收标准

1. `AppTray::poll` 能区分新增菜单动作与既有截图/退出动作。
2. 设置/历史动作打开对应独立窗口并聚焦；显示/隐藏只影响窗口可见性。
3. 关闭全部复用 `close_pin`，取消对应 OCR/导出 owner 任务并更新 runtime。
4. fmt/check/Clippy/workspace test、diff 检查和 ctx validate 通过。

## 验证

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：托盘菜单在无显示会话下不能创建；继续保持 `Option<AppTray>` 降级，不阻塞主循环。
- 风险：关闭多个贴图时迭代窗口 ID 可能和事件回调交错；动作在主循环收集后顺序执行。
- 回滚：仅撤销新增菜单项与动作分支，不改变窗口和 runtime 实现。

## 完成记录

- 2026-08-02：完成 `TrayAction` 扩展和主事件循环接线；设置/历史动作聚焦独立窗口，显示/隐藏/关闭全部贴图动作可用。
- 验证：`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check`、`ctx validate` 均通过。
- 已知风险：托盘为平台适配器实验路径，尚未完成 Windows/macOS/Linux 不同桌面环境的真实菜单、主题和无障碍探针。
