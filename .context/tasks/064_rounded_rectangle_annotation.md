# 任务 064：Overlay 圆角矩形标注

- 状态：已完成
- 计划：`.context/plans/064_rounded_rectangle_annotation.md`
- 规模：中
- 依赖：`.context/tasks/030_annotation_revision_contract.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/063_line_number_annotations.md`
- 生产行为变更：是；Overlay 新增圆角矩形描边工具。

## 任务目标

在当前受 tray 监督的 Overlay 内实现圆角矩形的拖拽预览、一次事务提交、撤销/重做和确定性合成。对象在提交时保存与当前线宽一致的半径，任何后续会话状态改变都不改写旧标注；全程不得出现任务栏/Dock 新窗口。

## 范围

- 为 `AnnotateTool`、`Annotation`、`DraftShape` 与 `AnnotateSession` 增加圆角矩形及冻结半径。
- 增加安全、抗锯齿的圆角矩形描边栅格化，并让草稿预览与最终烘焙复用。
- 接入工具栏、键盘快捷键和现有 Overlay 拖拽状态，补核心、UI 和窗口策略回归测试。
- 更新计划、任务、系统事实与风险记录。

## 非目标

- 不实现填充、透明度、可编辑半径面板、选择/修改已有标注、文本/模糊或平台窗口策略调整。
- 不改变现有矩形、其他标注、截图、贴图、OCR、导出、后台任务、公共 IPC 或持久化形状。

## 预期文件

- `crates/pinora-core/src/{annotate.rs,lib.rs}`
- `crates/pinora-app/src/{desktop_shell.rs,overlay_toolbar.rs,window_policy.rs}`
- `AGENTS.md`
- `.context/plans/064_rounded_rectangle_annotation.md`
- `.context/tasks/064_rounded_rectangle_annotation.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 有效拖拽只提交一条 `Annotation::RoundedRect` 并推进 revision；短边或取消草稿不提交。
2. 半径在提交时冻结、在极小矩形中安全钳制，反向和越界坐标不会导致越界像素访问。
3. 草稿预览与最终烘焙使用相同圆角渲染语义，颜色/线宽/边界可由纯像素测试复现。
4. 工具栏和快捷键均可切换圆角矩形；不会改变双击复制、重选、颜色取样或其他工具行为。
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

- 风险：小尺寸和大半径导致圆角相交、描边过密或越界。缓解：半径按短边钳制，距离场渲染只遍历安全边界，纯像素测试覆盖。
- 风险：旧图形跟随新线宽变化。缓解：半径作为 `Annotation::RoundedRect` 字段在提交时冻结。
- 风险：新增工具引入 Overlay 输入或任务栏/Dock 回归。缓解：复用现有拖拽和工具栏，执行 `window_policy` 源码守卫。
- 回滚：移除圆角矩形枚举、渲染和入口；其余工具、tray 和窗口策略保持不变。

## 完成记录

- 已完成：新增 `Annotation::RoundedRect` 与 `DraftShape::RoundedRect`；有效拖拽提交一个事务，宽或高不足两像素的草稿不推进 revision。
- 已完成：半径在提交时由线宽确定性派生并冻结；距离场描边在渲染时钳制到短边的一半，草稿预览与最终烘焙共享相同对象路径，线宽之后变化不会影响既有图形。
- 已完成：工具栏新增圆角矩形，`Q` 快捷键可切换；没有增加窗口、事件循环、平台菜单、截图或后台任务，`window_policy` 源码守卫保持有效。
- 已验证：核心圆角手势/冻结半径/预览/安全钳制、工具栏和 Overlay 定向测试，以及 fmt、workspace check、严格 Clippy 通过。全量离线测试、diff 检查和 `ctx validate` 在本任务最终门禁中执行。
- 已知风险：真实 GUI、HiDPI、复杂输入、绘制帧时间、tray 和任务栏/Dock 没有原生会话证据；未实现填充、透明度和可编辑半径面板。
