# 任务 062：Overlay 像素取色

- 状态：已完成
- 计划：`.context/plans/062_annotation_color_picker.md`
- 规模：中
- 依赖：`.context/tasks/030_annotation_revision_contract.md`、`.context/tasks/054_auxiliary_window_boundary.md`、`.context/tasks/061_tray_only_window_boundary.md`
- 生产行为变更：是；Overlay 工具栏新增截图内取色，点击后设置标注颜色并复制 HEX。

## 任务目标

在不创建窗口、不重捕获和不阻塞主线程的条件下，从当前 Overlay 选区的原始像素采样颜色，更新后续标注样式并通过当前会话的受监督剪贴板任务复制 `#RRGGBB`；工具栏布局适配新增入口。

## 范围

- 为 `AnnotateTool` 与 `AnnotateSession` 增加取色器、颜色设置和可测试颜色文本转换。
- 让 Overlay 的取色点击使用现有坐标映射和 `full_image` 原始像素，恢复之前的绘图工具，并提交受监督文本复制。
- 将工具栏改为由画布宽度决定的多行布局，增加取色图标与当前颜色色块绘制及布局测试。

## 非目标

- 不实现系统全局/贴图取色、放大镜、历史色板、取色透明度编辑或 RGB 格式切换。
- 不改变图片复制、OCR、标注烘焙、任务栏/Dock 策略、截图后端或公共持久化形状。

## 预期文件

- `crates/pinora-core/src/annotate.rs`
- `crates/pinora-app/src/{desktop_shell.rs,overlay_toolbar.rs}`
- `AGENTS.md`
- `.context/plans/062_annotation_color_picker.md`
- `.context/tasks/062_annotation_color_picker.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 有效像素采样能精确返回 RGBA 和稳定的 `#RRGGBB`；越界或无效缓冲受控拒绝。
2. 取色不创建 `Annotation`、不改变 revision；下一个图形标注使用采样颜色，取色器恢复前一个绘图工具。
3. 复制任务绑定当前 `JobOwner::Session` 与 `AssetRef`，重选/关闭/版本变化后不会交付。
4. 工具栏按钮在窄画布中换行且命中仍正确，绘制色块不越界；没有新增窗口构造路径。
5. 定向测试、fmt、workspace check、严格 Clippy、全量离线测试、diff 检查和 `ctx validate` 通过。

## 验证

- `cargo test -p pinora-core annotate -- --nocapture`
- `cargo test -p pinora-app overlay_toolbar::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：Overlay 映射与源码选区不一致会采样错误像素。缓解：复用 `overlay_annotate_local` 与 `active_src_rect`，由边界/缩放测试锁定。
- 风险：新的工具栏多行布局遮挡更多画布。缓解：先下方、再上方、最后画布内钳制；工具栏命中区域与布局纯函数测试覆盖。
- 风险：系统剪贴板失败。缓解：颜色已应用到当前标注，失败仅报告任务错误，不撤销取色或伪造成功。
- 回滚：移除取色工具和文本导出提交；保留既有颜色循环、标注与工具栏其他操作。

## 完成记录

- 已完成：核心新增 `sample_rgba_at`、`color_to_hex` 和 `AnnotateSession::set_color`；取色工具不会创建草稿或改变标注文档 revision。
- 已完成：Overlay 取色只读取 `full_image` 原始像素，依据 `overlay_annotate_local` 与 `active_src_rect` 做坐标换算；成功后恢复之前的绘图工具，并经 `ExportJobService` 复制 HEX。
- 已完成：工具栏新增取色器图标/当前颜色色块，并根据画布宽度分多行；极小画布下隐藏而不绘制裁切控件。
- 已验证：`cargo test -p pinora-core annotate -- --nocapture`、`cargo test -p pinora-app overlay_toolbar::tests -- --nocapture`、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`、fmt、workspace check、严格 Clippy、全量离线测试、diff 检查和 `ctx validate` 均通过。
- 已知风险：真实剪贴板成功、HiDPI、读屏、焦点和输入帧时间没有 GUI 会话证据；取色刻意不包含未提交的标注叠加。
