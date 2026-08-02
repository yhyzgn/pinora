# 任务 054：辅助窗口创建边界与托盘唯一常驻强化

- 状态：已完成
- 计划：`.context/plans/054_auxiliary_window_boundary.md`
- 规模：中
- 依赖：`.context/tasks/050_tray_only_windows.md`
- 生产行为变更：否；强化既有窗口策略的结构边界，保持用户可见语义不变。

## 范围

- 在 `window_policy` 提供唯一的辅助窗口创建和映射后策略入口。
- 迁移 desktop shell、历史、设置、Overlay、贴图及兼容会话的所有 `create_window` 调用。
- 补充窗口种类与映射后策略的离线契约测试，并更新经过验证的上下文事实和风险。

## 任务目标

防止未来窗口功能绕过任务栏/Dock 隔离：Pinora 进程空闲时只通过 tray、热键和 IPC 存活，用户操作期间出现的遮罩、贴图和面板不是新的任务栏/Dock 应用入口。

## 非目标

- 不改变窗口标题、尺寸、焦点、置顶、透明度、输入或关闭行为。
- 不新增其他 Wayland 合成器集成，不伪造真实桌面验收。
- 不修改 053 的贴图渲染缓存、像素、OCR、历史或导出实现。

## 预期文件

- `crates/pinora-app/src/{window_policy.rs,desktop_shell.rs,history_window.rs,settings_window.rs,region_overlay.rs,pin_window.rs}`
- `AGENTS.md`
- `.context/plans/054_auxiliary_window_boundary.md`
- `.context/tasks/054_auxiliary_window_boundary.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 全部生产 `create_window` 调用均经过 `window_policy::create_auxiliary_window`。
2. Overlay、贴图和面板在映射后统一执行 KWin 补充策略；隐藏 display-handle 仍以隔离属性创建但不触发映射后操作。
3. 空闲态无控制窗，托盘不可用时启动返回 `CapabilityUnavailable`；既有窗口交互不回退。
4. 定向测试、fmt、workspace check、严格 Clippy、全量测试、diff 检查与 `ctx validate` 通过。

## 验证

- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo test -p pinora-app kwin_place::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：映射后调用时机错误，KWin 找不到刚创建的窗口。缓解：只在可见后调用，保留 KWin 的异步延迟和贴图的原有同步放置流程。
- 风险：工厂遗漏兼容路径。缓解：审计全部 `create_window` 调用并以 `rg` 作为结构性验证。
- 风险：真实任务栏/Dock 行为仍受原生桌面环境影响。缓解：保持平台策略和 CI 编译覆盖，明确待 Windows/macOS/X11/KDE Wayland 探针。
- 回滚：移除工厂与映射后 API，恢复已验证的 050 调用形式；不改变用户数据或领域状态。

## 完成记录

- 2026-08-02：已将七个生产窗口构造路径收敛到 `window_policy::create_auxiliary_window`，并将 KWin 映射后策略收敛到同一模块。`rg` 审计确认业务模块不再直接调用 `create_window`；Overlay、贴图和面板均保留映射后策略，隐藏 display-handle 不映射。
- 已新增窗口种类与映射后策略契约测试；本地通过 `window_policy::tests`、`kwin_place::tests`、fmt、workspace check、严格 Clippy、全量 workspace 测试（app 138 通过、2 个真实桌面测试忽略；core 55 通过）、diff 检查与 `ctx validate`。
- 提交 `609b862` 的 GitHub CI `30734867309` 已在 Linux、macOS、Windows 原生 runner 通过格式、workspace 编译、严格 Clippy 和单元测试。真实任务栏、Dock、KWin、其他 Wayland 合成器与无障碍探针未覆盖。
