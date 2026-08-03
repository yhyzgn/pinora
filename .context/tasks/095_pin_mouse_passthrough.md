# 任务 095：贴图鼠标穿透

- 状态：已完成
- 计划：`.context/plans/095_pin_mouse_passthrough.md`
- 规模：中
- 依赖：060 贴图客户区菜单、066/061 tray-only 窗口策略、089/090 tray 贴图列表、095 前置的 `winit 0.30.13` 鼠标命中 API 事实。
- 生产行为变更：是；用户可将当前贴图设为鼠标穿透，并从现有 tray 贴图列表恢复该贴图交互。

## 任务目标

在不增加窗口或平台依赖的前提下，把窗口库已提供的协作式鼠标命中开关连接到现有贴图菜单和 tray 恢复路径，保证失败和陈旧操作不破坏可交互状态。

## 范围

- 为 `PinWin` 增加仅进程内的鼠标穿透状态；扩展 `PinContextMenu` 与 `TrayPinListEntry` 的无内容状态呈现。
- 在 `DesktopApp` 中封装请求/恢复鼠标命中、清理瞬态手势及 tray 激活恢复；平台失败只更新受控失败反馈。
- 扩展 `TrayFeedback` 的固定脱敏文案，并添加菜单、tray 标签、状态选择与失败保留回归。
- 更新当前工作指针、系统全景与风险。

## 预期文件

- `crates/pinora-app/src/{desktop_shell.rs,pin_context_menu.rs,tray.rs,tray_feedback.rs}`
- `AGENTS.md`
- `.context/plans/095_pin_mouse_passthrough.md`
- `.context/tasks/095_pin_mouse_passthrough.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不增加全局快捷键、持久化偏好、点击区域编辑、窗口透明度、动画、文件系统、截图/OCR/导出/历史行为或平台专有 FFI。
- 不修改核心领域、公共命令、窗口创建/展示工厂、任务监督、Pin 关闭恢复语义或任务栏/Dock/分页器策略。

## 验收标准

1. 只有平台命中关闭成功后贴图进入穿透；菜单、拖动、缩放和 OCR 选择等当前客户区瞬态状态被清除，失败时保持原状态。
2. tray 条目以无内容状态表明穿透；激活该条目仅在命中恢复成功后才显示/聚焦/重绘，重复和陈旧动作安全无副作用。
3. 穿透状态不会保存到设置、历史、领域、关闭恢复快照或日志；Copy/Save/OCR/锁定/置顶/可见性和 PinId 不改变。
4. 未新增窗口、事件循环、线程、外部进程、权限或敏感日志，原有 tray/window policy/workspace 回归不破坏。

## 验证

- `cargo test -p pinora-app --lib pin_context_menu::tests -- --nocapture`
- `cargo test -p pinora-app --lib tray::tests -- --nocapture`
- `cargo test -p pinora-app --lib tray_feedback::tests -- --nocapture`
- `cargo test -p pinora-app --lib desktop_shell:: -- --nocapture`
- `cargo test -p pinora-app --lib window_policy::tests -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：不同合成器对鼠标输入区域、焦点和 tray 激活的时序不同。缓解：以 API 实际返回为唯一状态门槛，失败保持原状态，tray 提供恢复入口；真实桌面验证不以离线测试替代。
- 风险：穿透后用户无法直接点击贴图。缓解：tray 条目稳定保留、无内容状态可见、激活先恢复命中再聚焦；关闭/隐藏不改变状态语义。
- 回滚：移除菜单项、临时状态、命中开关调用与 tray 状态标签即可恢复已有贴图行为；不触及图像、领域、设置、历史或窗口策略。

## 完成记录

- 完成时间：2026-08-03。
- 实现结果：`PASS` 通过窗口库命中开关把贴图设为仅当前生命周期的鼠标穿透状态。平台拒绝时不改变状态；成功后清除客户区菜单、拖动、缩放、OCR 选择和双击瞬态状态。tray 同一贴图条目先恢复命中，再显示、聚焦、重绘和更新最近使用。
- 回归覆盖：锁定/未锁定菜单均可发现 `PASS`；tray 标签不携带图像、路径、标题或内部 ID；平台失败保持原 `PinMouseMode`；反馈为固定脱敏文本且能力受限有稳定错误码。
- 门禁：`cargo fmt --all -- --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 280 通过、2 忽略；core 89 通过）、`cargo check --workspace --target x86_64-pc-windows-msvc`、`git diff --check` 与 `ctx validate` 全部通过。
- 风险：离线门禁不证明真实鼠标输入穿透、tray 刷新、焦点、HiDPI、性能或任务栏/Dock/分页器行为；详见 `.context/system/risks.md` 的 `R-053`。
