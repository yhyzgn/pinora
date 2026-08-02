# 任务 063：补齐直线与序号标注

- 状态：已完成
- 计划：`.context/plans/063_line_number_annotations.md`
- 规模：中
- 依赖：`.context/tasks/030_annotation_revision_contract.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/062_annotation_color_picker.md`
- 生产行为变更：是；Overlay 新增直线与自动递增的序号标记。

## 任务目标

在现有受 tray 监督的 Overlay 内实现可预览、可撤销的直线和连续序号标记。直线由一次有效拖拽提交，序号由一次有效点击立即提交；两个工具复用当前颜色/线宽、标注文档和渲染缓存，绝不创建任务栏/Dock 可见的新窗口。

## 范围

- 为 `AnnotateTool`、`Annotation`、`DraftShape` 和 `AnnotateSession` 增加直线/序号的受控手势、起始编号配置和事务语义。
- 为预览与烘焙路径增加确定性直线和序号栅格渲染，覆盖边界、退化输入和编号文本。
- 为 Overlay 工具栏和键盘增加入口，并确保序号单击不误入绘图拖拽或选区重选状态。
- 增加核心、工具栏、Overlay 及 `window_policy` 定向回归测试，更新任务完成记录与稳定风险事实。

## 非目标

- 不实现任意标注选择、调整、删除工具、圆角/填充、模糊、序号面板、持久化默认编号、贴图内编辑器重做或原生无障碍组件。
- 不修改窗口工厂、平台任务栏/Dock 策略、截图后端、资产数据形状、OCR/导出任务或公共 IPC。

## 预期文件

- `crates/pinora-core/src/{annotate.rs,lib.rs}`
- `crates/pinora-app/src/{desktop_shell.rs,overlay_toolbar.rs,window_policy.rs}`
- `AGENTS.md`
- `.context/plans/063_line_number_annotations.md`
- `.context/tasks/063_line_number_annotations.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 直线有效拖拽恰好提交一个 `Annotation::Line` 并推进 revision；短线或取消草稿不提交。
2. 序号每次有效点击恰好提交一个标记并递增；起始值受约束，撤销/重做或删除不重排先前提交的值。
3. 草稿预览与最终导出均采用相同绘制语义；颜色、线宽、图像边界和文字渲染可由纯测试验证。
4. 工具栏和快捷键可选中两个工具；序号点击不启动新选区、不创建任务或窗口。
5. `window_policy` 源码守卫继续拒绝策略模块外的事件循环或直接建窗，空闲 tray 常驻语义没有回退。
6. 定向测试、fmt、workspace check、严格 Clippy、全量离线测试、diff 检查和 `ctx validate` 均通过。

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

- 风险：序号单击与 Overlay 的双击复制/拖选竞争。缓解：在现有选区内优先消费序号点击，纯状态测试锁定不进入 `annotate_dragging`。
- 风险：编号溢出或撤销后重复编号导致标记含义不稳定。缓解：起始值限制为 `1..=99_999`，达到上限即停止新建标记；计数器只在成功提交后前进，不从文档逆推。
- 风险：自绘数字在极小尺寸或 HiDPI 下不易辨认。缓解：由最小直径和当前线宽派生稳定尺寸；真实桌面视觉/性能探针另行验证。
- 风险：新增 UI 入口绕过窗口策略。缓解：只修改现有 Overlay，执行源码级 `window_policy` 守卫。
- 回滚：删除直线/序号模型、绘制和入口；既有标注、工具栏、tray-only 窗口边界及资产流程不受影响。

## 完成记录

- 已完成：`Line` 使用一次有效拖拽创建单条标注，短线不推进 revision；`Number` 使用一次点击立即创建带固定值和直径的标记，编号从可配置起始值连续递增，达到 `99_999` 后停止，重设起点前不会重复提交。
- 已完成：直线/序号已接入最终烘焙和草稿预览的确定性渲染；工具栏及 `L`/`N` 快捷键均可选中，序号模式不会开启绘图拖拽，并显式排除双击复制路径。
- 已完成：未增加窗口、事件循环、平台菜单、截图或后台任务；`window_policy` 的生产源码扫描仍只允许策略模块构造事件循环和直接建窗。
- 已验证：核心标注、工具栏、序号双击和窗口策略定向测试，以及 fmt、workspace check、严格 Clippy、全量离线测试、diff 检查通过。全量结果为 app 168 通过、2 项真实桌面测试忽略；core 67 通过。
- 已知风险：没有真实 GUI 会话证据，不能将自绘数字、工具栏高 DPI、输入性能、tray 连续驻留或任务栏/Dock 行为描述为已验证。
