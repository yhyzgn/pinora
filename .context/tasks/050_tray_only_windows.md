# 任务 050：仅托盘常驻与辅助窗口任务栏隔离

- 状态：进行中
- 计划：`.context/plans/050_tray_only_windows.md`
- 规模：中
- 依赖：`.context/tasks/038_cross_platform_delivery.md`
- 生产行为变更：是；空闲控制窗移除，辅助窗口请求跳过任务栏/Dock。

## 范围

- 新增内部 `window_policy`，统一 Windows、X11 与 macOS 的辅助窗口/事件循环策略。
- 迁移 desktop shell、历史、设置、Overlay、贴图及兼容窗口创建路径。
- 移除 `desktop_shell` 中的空闲控制窗与其焦点处理；托盘、全局热键和 IPC 继续作为空闲入口。
- 增强 KWin 脚本，为 KDE Wayland 的 Pinora 辅助窗口设置 `skipTaskbar`。
- macOS app bundle 写入 agent 属性。

## 任务目标

让 Pinora 的后台常驻体验接近专业截图工具：用户只在托盘看到常驻入口，操作时只看见所需的遮罩或贴图内容，而非额外的任务栏窗口。

## 非目标

- 不通过模拟点击、外部窗口管理器配置或不可移植的系统服务实现其他 Wayland 合成器支持。
- 不把 CI 的 `--version` smoke 说成任务栏、Dock、焦点或真实 KDE 验证。

## 预期文件

- `crates/pinora-app/src/window_policy.rs`
- `crates/pinora-app/src/{lib.rs,desktop_shell.rs,history_window.rs,settings_window.rs,region_overlay.rs,pin_window.rs,kwin_place.rs}`
- `packaging/package-unix.sh`
- `AGENTS.md`
- `.context/plans/050_tray_only_windows.md`
- `.context/tasks/050_tray_only_windows.md`
- `.context/system/{overview.md,risks.md,conventions.md}`

## 验收标准

1. 空闲 desktop shell 无控制窗；已有托盘、IPC 和已注册热键入口正常保留。
2. 所有辅助 `WindowAttributes` 创建点使用统一策略；Windows/X11/macOS 编译条件被 CI 覆盖。
3. KDE KWin 脚本只按 Pinora 标题设置 `skipTaskbar`，脚本失败不会中断主流程。
4. macOS 包含 `LSUIElement`，且 event loop 使用 `Accessory` 激活策略。
5. 严格本地门禁与 GitHub 三平台 CI 成功；真实任务栏/Dock 和合成器验收缺口已记录。

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
- `gh workflow run ci.yml --ref main`（提交后使用对应 commit 的 CI run）

## 风险与回滚

- 风险：移除控制窗后，未注册全局热键的 Wayland 会话不能通过聚焦窗口按 F2；托盘和 IPC 保留，限制写入风险登记。
- 风险：Wayland 没有通用 `skip taskbar`；仅 KDE KWin 执行脚本，其他合成器不得宣称已支持。
- 风险：KWin 按标题匹配可能碰到同名窗口；仅使用 Pinora 固定/唯一标题并最小化脚本作用范围。
- 回滚：恢复控制窗与各窗口原属性；不影响持久化数据和业务工作流。

## 完成记录

- 2026-08-02：已删除 `ControlState`、控制窗口创建/隐藏/事件处理和启动自动截图路径；空闲态只轮询托盘、全局热键与 IPC。
- 已迁移 `desktop_shell`、`history_window`、`settings_window`、`region_overlay` 和 `pin_window` 的全部 `WindowAttributes` 创建点；macOS event loop 与 package plist 同步为 agent 语义。
- 已为 KWin 生成受标题限制的 `skipTaskbar`/`skipPager` 脚本，并增加转义、脚本内容与无效脚本 ID 的离线保护。
- 本地通过策略/脚本/Overlay 定向测试、fmt、workspace check、严格 Clippy、全量测试；待本提交的 GitHub CI。真实任务栏、Dock、KWin 与其他 Wayland 合成器探针未运行。
