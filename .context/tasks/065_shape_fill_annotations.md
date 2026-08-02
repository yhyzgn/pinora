# 任务 065：Overlay 封闭图形半透明填充

- 状态：已完成
- 计划：`.context/plans/065_shape_fill_annotations.md`
- 规模：中
- 依赖：`.context/tasks/030_annotation_revision_contract.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/064_rounded_rectangle_annotation.md`
- 生产行为变更：是；Overlay 矩形、圆角矩形和椭圆支持可切换的半透明填充。

## 任务目标

在当前受 tray 监督的 Overlay 内提供封闭图形的填充切换。填充状态只影响后续图形，提交时把半透明色冻结到对象；预览和最终导出共用同一渲染路径，且任何填充操作都不能产生任务栏/Dock 新窗口。

## 范围

- 为矩形、圆角矩形和椭圆增加可选冻结填充色，为 `AnnotateSession` 增加纯样式开关。
- 增加三类形状的 alpha 填充渲染，并确保填充在描边前执行。
- 在工具栏增加填充动作/选中态，在 Overlay 增加 `F` 快捷键；补充核心、工具栏、Overlay 与 `window_policy` 回归测试。
- 更新计划、任务、系统事实与风险记录。

## 非目标

- 不实现 alpha 滑块、渐变、图案、样式持久化、文本背景或已有标注编辑。
- 不改变非封闭工具、截图、贴图、OCR、导出、后台任务、公共 IPC、持久化形状或窗口工厂。

## 预期文件

- `crates/pinora-core/src/{annotate.rs,lib.rs}`
- `crates/pinora-app/src/{desktop_shell.rs,overlay_toolbar.rs,window_policy.rs}`
- `AGENTS.md`
- `.context/plans/065_shape_fill_annotations.md`
- `.context/tasks/065_shape_fill_annotations.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. `F` 与工具栏填充操作切换当前会话样式，不创建标注、不推进 revision、不提交任务或创建窗口。
2. 三类封闭图形提交时冻结可选半透明填充；关闭填充保持既有描边输出。
3. 预览与最终烘焙的 alpha 合成、填充边界和描边顺序一致，反向/边界坐标安全。
4. 工具栏明显反映填充开关，不妨碍其他工具命中或窄画布换行。
5. `window_policy` 源码守卫继续拒绝策略模块外的事件循环或直接建窗，tray-only 常驻语义没有回退。
6. 定向测试、fmt、workspace check、严格 Clippy、全量离线测试、diff 检查和 `ctx validate` 通过。

## 验证

- `cargo test -p pinora-core annotate -- --nocapture`
- `cargo test -p pinora-app overlay_toolbar::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：大面积 alpha 填充预览加重主线程绘制。缓解：只遍历封闭图形边界盒，复用现有预览缓存和 alpha 合成，不新增全图副本或 worker。
- 风险：颜色/开关变动重写已提交样式。缓解：填充色作为标注字段冻结，开关只影响下一个草稿。
- 风险：工具栏入口造成窗口策略回归。缓解：仅扩展现有客户区工具栏，执行 `window_policy` 源码守卫。
- 回滚：移除填充字段、渲染和工具栏/快捷键入口；描边图形、tray 和窗口策略保持不变。

## 完成记录

- 已实现：`AnnotateSession` 的填充开关默认关闭；`F` 与工具栏填充按钮只切换当前会话样式和工具栏选中态。矩形、圆角矩形、椭圆在提交时冻结 `[R, G, B, 96]` 填充色，预览与最终烧录均先填充、后描边，并统一经 `blend_coverage` 进行 alpha-over 合成。
- 已验证：`cargo test -p pinora-core annotate -- --nocapture`（20 通过）、`cargo test -p pinora-app overlay_toolbar::tests -- --nocapture`（6 通过）、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`（25 通过）、`cargo test -p pinora-app window_policy::tests -- --nocapture`（3 通过）、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 169 通过/2 忽略，core 71 通过）、`git diff --check` 和上下文校验。
- 未覆盖风险：上述离线验证不证明真实 Windows/macOS/X11/KDE Wayland 的任务栏/Dock、托盘、HiDPI、读屏或连续大面积填充拖拽帧时间；这些仍需原生桌面会话证据。
