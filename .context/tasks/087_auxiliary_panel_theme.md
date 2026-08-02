# 任务 087：辅助面板主题渲染

- 状态：已完成
- 计划：`.context/plans/087_auxiliary_panel_theme.md`
- 规模：大
- 依赖：v3 `ThemeMode` 持久化、设置/历史/诊断 `Panel` 适配器。
- 生产行为变更：设置、历史和诊断面板实际按 System/Light/Dark 渲染，并在成功保存或系统主题事件后刷新。

## 任务目标

建立共享自绘调色板，将现有设置值从“仅保存”变为可见辅助面板主题，同时保持设置提交的原子语义和 tray-only 窗口生命周期。

## 范围

- 新增 `crates/pinora-app/src/panel_theme.rs`：纯主题解析、颜色 token、系统外观映射和测试。
- 修改设置、历史、诊断的模型/绘制/窗口适配器及 DesktopApp 保存/主题事件接线。
- 更新 `AGENTS.md`、087 计划/任务、`.context/system/{overview.md,risks.md}`。

## 预期文件

- `crates/pinora-app/src/{panel_theme.rs,settings_panel.rs,settings_window.rs,history_browser.rs,history_window.rs,diagnostics_panel.rs,diagnostics_window.rs,desktop_shell.rs,lib.rs}`
- `AGENTS.md`
- `.context/plans/087_auxiliary_panel_theme.md`
- `.context/tasks/087_auxiliary_panel_theme.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不为 Overlay、贴图、上下文菜单、tray 或标题栏实现主题；不新增依赖或更改 `ThemeMode` 的数据形状。
- 不写系统设置、不依赖环境变量探测主题，不改变窗口创建/显示策略或运行时能力语义。

## 验收标准

1. 共享解析明确覆盖 Light/Dark/System/未知系统外观，且主题 token 可被三种面板共同使用。
2. 设置中的主题草稿立即改变自身渲染；原子保存成功才刷新已打开历史/诊断，失败不刷新。
3. 窗口创建时读取系统外观，收到 `ThemeChanged` 时仅在 `System` 偏好下更新相关面板。
4. 不新增辅助窗口、事件循环或后台资源；所有现有 `window_policy` 守卫和业务回归保持通过。

## 验证

- `cargo test -p pinora-app panel_theme -- --nocapture`
- `cargo test -p pinora-app settings_panel -- --nocapture`
- `cargo test -p pinora-app history_browser -- --nocapture`
- `cargo test -p pinora-app diagnostics_panel -- --nocapture`
- `cargo test -p pinora-app window_policy -- --nocapture`
- `cargo test -p pinora-app desktop_shell -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：主题草稿在保存失败时泄漏到运行时面板。缓解：设置窗口独占草稿外观；历史/诊断只在成功分支接收持久化主题。
- 风险：未获系统外观时浅深切换不确定。缓解：`System` 未知稳定回退 Dark，明确测试。
- 风险：颜色替换破坏可读性或按钮状态。缓解：集中 token、浅深帧差异测试与现有状态/布局回归；真实读屏和色彩对比度仍需桌面验收。
- 回滚：删除共享主题模块与刷新接线，恢复原固定深色；设置 schema、业务行为和窗口策略不变。

## 完成记录

- 已验证事实：`PanelTheme` 为设置、历史和诊断提供 Light/Dark/System token；设置草稿切换立即改变自身帧；历史/诊断主题仅由成功的原子保存结果发布；三个窗口保持既有 `window_policy` 创建与展示，并在 System 偏好下处理 `ThemeChanged`。
- 验证结果：定向主题、设置、历史、诊断、`desktop_shell` 与 `window_policy` 测试通过；`cargo fmt --check`、workspace 编译、严格 Clippy、离线 workspace 测试（app 240 通过、2 忽略；core 88 通过）、Windows target 编译、`git diff --check` 与 `ctx validate` 通过。
- 未知与风险：本机未运行真实桌面主题切换或 GUI/读屏/HiDPI/窗口管理器探针；这些不能由单元测试、CI 或 target 编译替代。
