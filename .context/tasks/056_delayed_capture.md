# 任务 056：可取消延迟截图与 Pinora 自隐藏恢复

- 状态：已完成
- 计划：`.context/plans/056_delayed_capture.md`
- 规模：中
- 依赖：`.context/tasks/054_auxiliary_window_boundary.md`、`.context/tasks/055_display_targeted_capture.md`
- 生产行为变更：是；tray 新增 1/3/5 秒延迟区域截图和取消操作，倒计时期间隐藏已显示贴图。

## 范围

- 扩展 tray 菜单/动作以启动和取消延迟区域截图。
- 在 desktop shell 增加受控倒计时状态、贴图可见性快照和唯一恢复路径。
- 暂停/恢复 `FrameCache`，确保倒计时不会消费可能包含 Pinora 的预截帧。
- 补充状态机与 tray 离线测试，更新经过验证的上下文和风险。

## 任务目标

让用户能在打开菜单、弹窗或需要暂时清空 Pinora 贴图后完成截图，而不让延迟功能引入退出、残留隐藏贴图、额外任务栏窗口或错误缓存。

## 非目标

- 不实现倒计时 UI、声音、通知、全局取消快捷键、可配置延迟、延迟全屏或窗口截图。
- 不隐藏其他应用窗口、强制关闭 tray 菜单或绕过系统捕获权限。
- 不改变既有即时截图、区域选区、全屏、贴图、OCR、历史和导出契约。

## 预期文件

- `crates/pinora-app/src/{tray.rs,desktop_shell.rs}`
- `AGENTS.md`
- `.context/plans/056_delayed_capture.md`
- `.context/tasks/056_delayed_capture.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. tray 1/3/5 秒动作与取消动作映射稳定；没有状态时取消不影响即时截图。
2. 倒计时只隐藏原先可见贴图，暂停缓存；到期后的真实捕获前不恢复。
3. 成功像素获取、取消、启动失败和退出分别恢复可见贴图且回到正确状态，不退出 tray 进程。
4. 不创建任何新的常驻或辅助窗口，054 的任务栏/Dock 约束不回退。
5. 定向测试、fmt、workspace check、严格 Clippy、全量测试、diff 检查与 `ctx validate` 通过。

## 验证

- `cargo test -p pinora-app tray::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：恢复路径遗漏导致贴图永久隐藏。缓解：状态机统一拥有可见窗口 ID 快照，并覆盖成功、取消、失败和退出。
- 风险：倒计时仍使用旧缓存。缓解：开始即暂停并清空 `FrameCache`，到期后只走冷捕获。
- 风险：系统 tray 菜单关闭时机无法验证。缓解：不创建倒计时窗口，仅保证 Pinora 自己窗口已隐藏；真实桌面行为单独验证。
- 回滚：删除倒计时动作和状态机，恢复立即区域截图；不触及用户数据和核心捕获协议。

## 完成记录

- 已实现 tray `CaptureRegionAfter(Duration)` / `CancelDelayedCapture` 动作与 1/3/5 秒菜单映射；倒计时激活时取消项可用，重复延时动作被禁用。
- 已实现 `DelayedCapture` 状态、冷捕获分支和贴图可见状态快照；恢复仅操作倒计时开始时可见且尚未关闭的 `WindowId`，不会错误显示先前隐藏的贴图。
- 已验证：`cargo test -p pinora-app tray::tests -- --nocapture`（3 通过）、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`（19 通过）、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 147 通过、2 忽略；core 55 通过）、`git diff --check`。
- 未覆盖风险：没有真实 GUI 会话，因此不将离线状态机和静态窗口策略当作 Windows/macOS/X11/KDE Wayland 的任务栏、Dock、托盘或合成器验收。
