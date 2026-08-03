# 任务 090：贴图最近使用排序

- 状态：已完成
- 计划：`.context/plans/090_pin_recency_order.md`
- 规模：小
- 依赖：089 的动态 tray 贴图列表、`desktop_shell.rs` 的 Pin 焦点事件与 `PinWin` 所有权。
- 生产行为变更：是；tray 中当前贴图从固定身份排序改为最近使用优先排序。

## 任务目标

把多贴图的可发现性从“创建顺序”提升为“最近使用顺序”，而不把 UI 短期状态泄漏到领域或磁盘。

## 范围

- 向 `TrayPinListEntry` 增加仅内部使用的 recency 排序键，并按 recency 降序、PinId 回退排序。
- 向 `PinWin` 和 `DesktopApp` 添加进程内饱和 recency 计数；在新建、获取焦点和 tray 唤起时更新。
- 焦点事件更新后刷新现有 tray 子菜单；补充排序、并列和饱和契约测试。
- 更新当前工作指针、计划、任务、系统全景和风险。

## 预期文件

- `crates/pinora-app/src/{tray.rs,desktop_shell.rs}`
- `AGENTS.md`
- `.context/plans/090_pin_recency_order.md`
- `.context/tasks/090_pin_recency_order.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不增加或改写窗口、线程、截图、OCR、导出、设置、历史、领域命令、持久化和平台 API。
- 不显示/记录最近使用时间、PinId、title、图像、OCR、路径或坐标。
- 不把真实跨平台焦点和 tray 动态更新的未验证行为包装为已验收。

## 验收标准

1. list 快照按 recency 降序、PinId 升序确定性排序；标签保持通用序号和可见性。
2. 新建、`Focused(true)` 和 `ActivatePin` 都只更新现有进程内值并刷新菜单；饱和计数不会回绕或 panic。
3. 既有单贴图唤起仍只经 `window_policy` 显示、聚焦和重绘，不新建窗口。
4. 现有 tray、desktop shell、window policy 和 workspace 回归不破坏。

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

- 风险：系统焦点事件缺失、延迟或拒绝。缓解：tray 唤起主动更新排序，普通焦点事件仅作补充；真实平台单独验收。
- 风险：排序依赖 HashMap 或 recency 溢出导致条目跳动。缓解：排序键明确、PinId 回退、饱和递增，并用纯测试锁定。
- 风险：焦点处理绕开窗口策略。缓解：只在 089 既有 `ActivatePin` 的展示后记录 recency，焦点事件只更新内存与菜单。
- 回滚：删除 recency 字段、更新调用和排序键，即恢复 089 行为；不影响已有贴图、tray、截图或持久化。

## 完成记录

- 已实现无持久化 `last_used` 排序键和饱和 clock：新建、真实 `Focused(true)` 和 tray 唤起均更新既有贴图的内存 recency；未知窗口不会改变 clock。tray 以最近使用降序、PinId 升序重排，标签继续仅含通用序号和可见性。
- 已补充并列回退、最近使用优先和饱和计数纯测试；单贴图唤起仍只经 `window_policy` 显示、聚焦和重绘，没有新窗口/线程/后台工作。
- 验证：`cargo fmt --check`；`cargo test -p pinora-app tray -- --nocapture`（19 通过）；`cargo test -p pinora-app desktop_shell -- --nocapture`（33 通过）；`cargo test -p pinora-app window_policy -- --nocapture`（4 通过）；`cargo check --workspace`；`cargo clippy --workspace --all-targets -- -D warnings`；`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 251 通过、2 忽略；core 88 通过）；`cargo check --workspace --target x86_64-pc-windows-msvc`；`git diff --check`；`ctx validate` 均通过。
- 已知风险：真实原生焦点事件、tray 菜单刷新、Windows/macOS/X11/KDE Wayland 的任务栏/Dock/分页器隔离和高 DPI 尚未验证，见 `R-048`。
