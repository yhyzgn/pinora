# 任务 089：托盘贴图列表与单贴图唤起

- 状态：已完成
- 计划：`.context/plans/089_tray_pin_list.md`
- 规模：中
- 依赖：`tray.rs` 当前静态菜单、`desktop_shell.rs` 的 `PinWin` 所有权和 `window_policy`。
- 生产行为变更：是；托盘新增当前贴图列表，用户可显示并聚焦一个既有贴图。

## 任务目标

让 tray 成为贴图的可发现、可定位入口，但不让它成为新的窗口生命周期或领域状态 owner。

## 范围

- 在 `AppTray` 增加可动态重建的“贴图列表”子菜单、结构化列表项和 `ActivatePin(PinId)` 菜单动作。
- 从 `DesktopApp` 的现有 `pins` 汇总无敏感列表快照，创建、关闭、显示/隐藏和编辑可见性变化后同步 tray。
- 选择单贴图时复用 `window_policy::show_auxiliary_window`，随后聚焦、重绘同一个已存在窗口。
- 为静态映射、标签安全、状态同步和未知 ID 忽略补充纯测试，并更新计划、任务、系统全景和风险。

## 预期文件

- `crates/pinora-app/src/tray.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `AGENTS.md`
- `.context/plans/089_tray_pin_list.md`
- `.context/tasks/089_tray_pin_list.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不增加截图、Overlay、历史、设置、OCR、导出、Pin 编辑、持久化或全局热键能力。
- 不展示 Pin 标题、图像元数据、OCR、路径、内部 ID、位置、大小或缩略图；不创建预览窗口。
- 不改变批量贴图动作、最近关闭恢复、窗口层级、任务栏/Dock/分页器策略或平台后端。

## 验收标准

1. tray 动态显示“贴图列表”子菜单：无贴图时有禁用占位项；有贴图时按稳定 PinId 排序，标签仅含顺序号与可见性状态。
2. 菜单选择准确生成 `TrayAction::ActivatePin(PinId)`；未知菜单 ID 和已关闭 PinId 不会 panic 或创建新窗口。
3. 激活路径只复用匹配的 `PinWin`，将其置为可见、经 `window_policy` 展示、请求焦点与重绘。
4. 各现有可见性/生命周期入口均刷新列表；仍无新增窗口/线程/依赖/后台路径，窗口策略守卫和现有业务行为不回归。

## 验证

- `cargo test -p pinora-app tray -- --nocapture`
- `cargo test -p pinora-app desktop_shell -- --nocapture`
- `cargo test -p pinora-app window_policy -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：部分 tray 实现延迟应用子菜单更新。缓解：只在 GUI 线程更新既有 `Submenu`，菜单失败仅记录受控错误，绝不修改贴图状态；真实桌面单独验收。
- 风险：错误地按 title、图像内容或 HashMap 顺序生成标签会泄露信息或造成条目跳动。缓解：仅以 PinId 做稳定排序，显示以列表顺序产生的通用标签。
- 风险：错误的激活路径可能绕开窗口策略或新建窗口。缓解：只经 `show_auxiliary_window` 操作现存 `PinWin`，并添加源码/行为测试。
- 回滚：删除动态子菜单、列表同步和 `ActivatePin` 分支；既有 tray 菜单和贴图窗口生命周期不受影响。

## 完成记录

- 动态子菜单、空列表禁用占位、稳定排序、通用无内容标签及 `ActivatePin(PinId)` 映射均已实现；条目标签不会包含 Pin title、图像、OCR、路径、坐标或内部 ID。
- `DesktopApp` 仅对已有 `PinWin` 执行显示、焦点和重绘，展示继续唯一经过 `window_policy`；未知 PinId 安全忽略。创建、关闭、批量显示/隐藏、延时截图和编辑可见性路径均同步菜单，批量关闭只重建一次。
- 验证：`cargo fmt --check`；`cargo test -p pinora-app tray -- --nocapture`（18 通过）；`cargo test -p pinora-app desktop_shell -- --nocapture`（32 通过）；`cargo test -p pinora-app window_policy -- --nocapture`（4 通过）；`cargo check --workspace`；`cargo clippy --workspace --all-targets -- -D warnings`；`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 249 通过、2 忽略；core 88 通过）；`cargo check --workspace --target x86_64-pc-windows-msvc`；`git diff --check`；`ctx validate` 均通过。
- 已知风险：真实 tray 菜单刷新、点击后的焦点、Windows/macOS/X11/KDE Wayland 的任务栏/Dock/分页器隔离与高 DPI 尚未验证，见 `R-048`。
