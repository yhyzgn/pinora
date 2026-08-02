# 任务 076：贴图边缘缩放与适应原图

- 状态：已完成
- 计划：`.context/plans/076_pin_resize_and_fit.md`
- 规模：中
- 依赖：`.context/tasks/053_pin_render_cache.md`、`.context/tasks/060_pin_context_menu_editing.md`、`.context/tasks/066_auxiliary_window_visibility_policy.md`
- 生产行为变更：是；既有贴图支持四边/四角等比缩放与 100% 原图恢复。

## 任务目标

在不创建任何新窗口的前提下，为当前贴图增加八方向客户区尺寸热区和“适应原图”操作。尺寸计算保持比例、范围、锚点和缓存失效的一致性；窗口及进程仍严格 tray-only。

## 范围

- 扩展 `pin_layout` 的纯几何、命中和尺寸恢复规则及测试。
- 接入 `desktop_shell` 当前贴图的指针、键盘/菜单动作、尺寸/位置请求和 transform 同步。
- 扩展 `pin_context_menu` 的已启用动作和客户区菜单渲染/命中。
- 覆盖热区、锁定、反向/边界、缓存、领域状态和窗口策略，更新项目上下文。

## 非目标

- 不实现点击穿透、旋转、非等比缩放、原生窗口边框缩放、分组/标签、跨进程恢复、OCR 模型/文字层、截图、导出、历史或设置改造。
- 不创建或展示新窗口、系统菜单、工具提示、Toast、截图或 worker。

## 预期文件

- `crates/pinora-app/src/{desktop_shell.rs,pin_layout.rs,pin_context_menu.rs}`
- `AGENTS.md`
- `.context/plans/076_pin_resize_and_fit.md`
- `.context/tasks/076_pin_resize_and_fit.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 当前未锁定贴图的四边/四角热区可保持图像比例缩放；最小、最大、边界和反向拖动都产生稳定有效尺寸和锚点位置。
2. 100% 恢复仅调整当前贴图窗口与现有 transform；锁定贴图拒绝尺寸变换，其他既有操作不受影响。
3. 尺寸变化仅在实际变化时失效渲染缓存、请求重绘并同步领域 `PinTransform`；不创建新窗口或后台任务。
4. 所有窗口继续隐藏创建、唯一展示；空闲只有 tray，辅助层禁止任务栏、Dock 与分页器项。

## 验证

- `cargo test -p pinora-app pin_layout -- --nocapture`
- `cargo test -p pinora-app pin_context_menu -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：resize 热区干扰拖动或 OCR 选择。缓解：固定优先级、命中边界和纯几何回归测试。
- 风险：连续拖动导致缓存反复重建。缓解：只在尺寸变化时更新，保留既有缓存键和无关重绘隔离。
- 风险：平台合成器对无边框尺寸/位置请求或任务栏/Dock 隔离与离线模型不同。缓解：不添加建窗/展示路径，保持 `window_policy` 守卫，并将真实桌面验收保留为开放风险。
- 回滚：删除边缘热区、尺寸拖动和菜单恢复动作；保留既有贴图拖动、滚轮缩放、锁定、OCR、关闭撤销与 tray-only 策略。

## 完成记录

- 已完成：新增八方向比例缩放纯逻辑和边界回归；边/角均维持原图比例，缩放收敛到既有 `0.05..=8.0` 领域范围，手动回退路径保持固定对边/对角锚点。
- 已完成：贴图仅在未锁定、非 OCR Ctrl+拖选且未打开客户区菜单时响应边缘命中；原生 resize 使用平台协议，失败才请求既有窗口的新尺寸。鼠标形状按边/角变化并去重，不在图片上常驻绘制控制块。
- 已完成：双击客户区及右键菜单 `100%` 都只复用当前贴图窗口恢复 `scale = 1.0`。锁定贴图拒绝缩放和恢复，关闭、复制、OCR、菜单、滚轮缩放、历史重新贴图与 tray-only 策略保持原行为。
- 已验证：`cargo test -p pinora-app pin_layout -- --nocapture`（8 通过）、`cargo test -p pinora-app pin_context_menu -- --nocapture`（3 通过）、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`（30 通过）、`cargo test -p pinora-app window_policy::tests -- --nocapture`（4 通过）；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 191 通过、2 忽略；core 85 通过）、`git diff --check` 和 `ctx validate` 全部通过。
- 未验证：没有 GUI 端到端或真实桌面会话证据，不能把离线门禁表述为 Windows、macOS、X11、KDE Wayland 的任务栏/Dock/分页器隔离、原生 resize、焦点、首帧或高 DPI 性能已验收。
