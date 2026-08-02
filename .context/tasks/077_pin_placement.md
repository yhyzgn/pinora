# 任务 077：贴图默认避让截图来源

- 状态：已完成
- 计划：`.context/plans/077_pin_placement.md`
- 规模：小
- 依赖：`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/066_auxiliary_window_visibility_policy.md`、`.context/tasks/076_pin_resize_and_fit.md`
- 生产行为变更：是；普通 Overlay 新贴图会优先避开截图源区域。

## 任务目标

在不新建任何窗口的前提下，为普通截图完成后的新贴图计算默认初始位置。算法只使用当前 Overlay 已持有的完整捕获范围和源区域，以物理像素决定右、左、下、上的无重叠候选；没有候选时安全回退。

## 范围

- 扩展 `pin_layout` 的纯位置选择规则与回归测试。
- 在 `desktop_shell` 的普通 `OverlayFinish::Pin` 路径接入完整捕获范围和默认位置。
- 更新工作指针、项目上下文与风险登记。

## 非目标

- 不实现多贴图碰撞规避、自动排列、动画、跨屏猜测、点击穿透、分组/标签、历史位置迁移或新的窗口类型。
- 不修改贴图编辑、历史重新贴图、关闭撤销、截图、标注、OCR、导出、tray、设置或窗口可见性策略。

## 预期文件

- `crates/pinora-app/src/{desktop_shell.rs,pin_layout.rs}`
- `AGENTS.md`
- `.context/plans/077_pin_placement.md`
- `.context/tasks/077_pin_placement.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 当前普通 Overlay 新建贴图在有空间时完整位于当前捕获范围并与截图源区域不相交，候选顺序为右、左、下、上。
2. 负坐标、大图、全屏和没有可用候选时返回稳定合法坐标，不自动缩放、创建窗口或猜测其他显示器。
3. 历史重新贴图、贴图编辑、关闭撤销与现有拖动/缩放/OCR/tray-only 行为保持不变。
4. 所有窗口继续隐藏创建、唯一展示；空闲只有 tray，辅助层禁止任务栏、Dock 与分页器项。

## 验证

- `cargo test -p pinora-app pin_layout -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：可用范围不足时仍遮挡源区域。缓解：候选完整容纳和不相交纯测试，明确稳定回退，不伪造满足避让。
- 风险：坐标边界或负坐标溢出。缓解：在纯逻辑使用宽位中间计算，所有返回点可表示于现有物理坐标空间。
- 风险：平台窗口管理器忽略初始位置或辅助窗口仍进入任务栏/Dock/分页器。缓解：不新增建窗/展示路径，保持 `window_policy` 守卫，并保留真实桌面验收风险。
- 回滚：移除位置选择函数及调用，恢复源区域左上坐标；不影响图像、PinId、OCR、导出、历史、tray 或窗口策略。

## 完成记录

- 已完成：在 `pin_layout` 实现纯 `default_pin_position`，按右、左、下、上优先级选择完整容纳且不相交的初始位置；负坐标、全屏回退和超大图像均保持确定性与边界安全。
- 已完成：`desktop_shell` 只在普通 Overlay 的 `OverlayFinish::Pin` 分支调用该策略，并使用当前 `full_image.source_rect` 作为唯一已知捕获范围。复制、保存、贴图编辑、历史重新贴图与关闭撤销没有改变。
- 已完成：没有新增窗口、事件循环、显示调用、截图、worker 或系统菜单；tray-only 和 `window_policy` 守卫持续有效。
- 已验证：`cargo test -p pinora-app pin_layout -- --nocapture`（14 通过）、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`（30 通过）、`cargo test -p pinora-app window_policy::tests -- --nocapture`（4 通过）、格式、workspace 编译、严格 Clippy 与全量离线测试（app 196 通过、2 忽略；core 85 通过）通过。
- 未验证：真实桌面合成器可如何解释初始无边框窗口位置、窗口映射、任务栏/Dock/分页器隔离、焦点和高 DPI 性能，不能由离线门禁证明。
