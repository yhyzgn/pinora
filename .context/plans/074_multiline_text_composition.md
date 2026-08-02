# 计划 074：Overlay 多行文本与明确提交边界

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/074_multiline_text_composition.md`

## 目标

落实设计文档 4.4 与 6.2 的文本标注基本编辑契约：文本草稿支持 `Shift+Enter` 显式换行，提交后的多行文本按固定行距渲染与命中；用户通过外部重选或工具切换离开非空文本草稿时必须先提交，禁止隐式丢失。实现只能使用当前 Overlay 与现有标注文档事务，Pinora 空闲继续只驻留系统 tray，任何辅助层均不得进入任务栏、Dock 或分页器。

## 非目标

- 不实现富文本、字体选择、背景、自动换行、文本框尺寸调整、已提交文本二次编辑、输入法候选窗控制、旋转或缩放。
- 不改变标注存储格式、导出/OCR/贴图输入、截图、窗口工厂、系统菜单、后台任务或事件循环。

## 约束

- 换行必须保留在 `Annotation::Text.content` 中；渲染和命中使用同一行距规则，空白行也占据行高，不能把 `\n` 静默丢弃。
- `Enter` 继续提交文本，`Shift+Enter` 仅插入换行且不推进 revision；`Esc` 继续作为显式取消草稿。
- 外部重选与工具切换若遇到文本草稿，必须通过既有一次 `AnnotationDoc` 事务提交非空文本；空白文本可安全丢弃。提交不得重新捕获、创建窗口或启动 worker。
- 所有表现仍在当前 Overlay 帧内完成；不增加窗口、可见性调用、事件循环、系统菜单、截图或后台任务。

## 依赖关系

- 依赖 `pinora-core::AnnotateSession`、`AnnotationDoc` revision/undo/redo 事务与字体栅格化路径。
- 依赖 068 的预览缓存失效规则和 070/071 的选择/移动命中边界。
- 依赖 061/066 的 tray-only GUI 会话及辅助窗口创建、展示守卫。

## 阶段

1. 在 core 收敛换行、行距、文本 bounds 和渲染规则，并以纯逻辑/像素测试锁定。
2. 在现有 Overlay 输入中区分 `Shift+Enter` 与 `Enter`，对外部重选和工具切换落实文本草稿提交。
3. 覆盖 revision、撤销/重做、选择命中、预览缓存、窗口策略与全量离线门禁，更新上下文和风险。

## 检查点

1. 多行文本在预览、提交、导出和命中路径具有一致的行距与像素边界。
2. `Shift+Enter` 不提交，`Enter` 提交一次，外部重选/工具切换不丢失非空文本；空白草稿不污染文档。
3. 功能仅复用既有 Overlay、标注文档和渲染缓存；没有新窗口、截图、任务或可见性调用。

## 计划级风险

- 换行的 bounds 与实际字形不一致会导致选择、移动或局部预览裁剪错误；必须让文本 bounds 和绘制共用行距并覆盖空白行。
- 自动提交时机错误可能造成重复事务、意外提交或 redo 分支损坏；必须以 revision 与 undo/redo 回归锁定。
- 离线字体加载、点阵/字体渲染、CI 和源码守卫不能证明真实 Windows、macOS、X11、KDE Wayland 的输入法、焦点、输入延迟、任务栏/Dock/分页器隔离；不得外推为原生桌面证据。

## 变更前记录

```text
目的：让文本标注支持实际的多行内容，并保证非空输入不会在继续操作时隐式丢失。
影响路径：标注文本渲染/命中、Overlay 键盘与鼠标编辑边界、核心/App 测试、上下文文档。
兼容性：保持 Annotation::Text 的既有 content 字段与单行行为；不改变导出格式、资产、IPC、权限或窗口生命周期。
外部副作用：无网络、外部进程、截图、新窗口、系统菜单或后台任务；仅更新当前 Overlay 的内存文档与呈现帧。
回滚点：移除换行处理和草稿自动提交分支，恢复单行文本输入；保留既有文本、标注、导出、tray 与窗口策略。
验证场景：Shift+Enter/Enter/Esc、空白/多行草稿、外部重选/工具切换、bounds/命中、undo/redo、预览/导出一致、window policy 与全量离线门禁。
```

## 完成标准

- 文本草稿支持多行，预览、提交、导出、选择命中、移动和撤销/重做一致；用户不会因外部重选或工具切换隐式丢失非空文本。
- 核心文本规则和 Overlay 输入边界均有回归；没有新增窗口、事件循环、系统菜单、截图或 worker。
- 定向测试、fmt、workspace check、严格 Clippy、全量离线测试、差异检查和 `ctx validate` 通过；真实桌面输入法、字体、缩放、帧时间与 tray/任务栏/Dock/分页器行为明确保留。

## 完成记录

- 已完成：`Annotation::Text.content` 中的换行不再被绘制或 bounds 忽略。文本绘制、fallback 占位、选择 bounds 和命中统一使用固定行距，空白行也保留可选择的垂直空间；预览、提交、导出、移动和撤销/重做继续复用同一标注文档与渲染路径。
- 已完成：现有 Overlay 将 `Shift+Enter` 映射为不推进 revision 的草稿换行，`Enter`/`Ctrl+Enter` 提交，`Esc` 保持显式取消。外部重选或切换至其他工具前会提交非空文本草稿；空白草稿会在同一次外部点击中安全清除，不产生事务。
- 已验证：核心多行/空白行/bounds/命中/revision/undo/redo、Overlay Enter 分流与既有窗口策略回归，以及 `cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check` 和 `ctx validate` 通过。全量离线结果为 app 183 通过、2 忽略，core 85 通过。
- 未验证：真实 Windows、macOS、X11、KDE Wayland 的输入法、字体回退、复杂文本、1x/2x DPI、输入延迟、焦点、tray 连续驻留以及辅助窗口绝不进入任务栏、Dock 或分页器的窗口管理器行为。
