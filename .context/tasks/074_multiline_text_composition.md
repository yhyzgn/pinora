# 任务 074：Overlay 多行文本与明确提交边界

- 状态：已完成
- 计划：`.context/plans/074_multiline_text_composition.md`
- 规模：中
- 依赖：`.context/tasks/009_region_overlay.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/066_auxiliary_window_visibility_policy.md`、`.context/tasks/068_overlay_preview_cache.md`、`.context/tasks/070_annotation_selection_delete.md`
- 生产行为变更：是；文本标注支持 `Shift+Enter` 多行，非空草稿不会在重选或工具切换时被隐式丢弃。

## 任务目标

在保持当前 `Annotation::Text.content` 数据形状和无新窗口约束下，统一多行文本的渲染、bounds 与命中，并让 `Shift+Enter` 插入换行、`Enter` 提交。离开非空文本草稿时先形成既有单个标注事务，避免用户输入丢失。

## 范围

- 在 `pinora-core` 增加多行文本行距、渲染与 bounds 一致性。
- 在现有 Overlay 键盘/鼠标/工具切换路径补齐显式提交边界。
- 覆盖文本 revision、撤销/重做、选择命中、预览与窗口策略回归。
- 更新计划、任务、系统事实和风险记录。

## 非目标

- 不实现富文本、自动换行、文本框调整、背景、字体选择、已提交文本编辑或新的文字窗口。
- 不改截图、贴图、导出、OCR、历史、设置、窗口策略、系统菜单或 worker。

## 预期文件

- `crates/pinora-core/src/annotate.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `AGENTS.md`
- `.context/plans/074_multiline_text_composition.md`
- `.context/tasks/074_multiline_text_composition.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. `Shift+Enter` 只插入换行，`Enter` 将非空文本提交为一次事务；`Esc` 取消草稿，空白草稿不推进 revision。
2. 多行文本预览、提交、导出、bounds、选择/移动及 undo/redo 使用相同行距；外部重选和工具切换不会丢弃非空文本。
3. 实现仅使用已有 Overlay 和标注文档；不创建/展示/管理新窗口，不启动事件循环、系统菜单、截图或 worker，空闲 Pinora 继续只驻留 tray。

## 验证

- `cargo test -p pinora-core annotate -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：多行渲染/边界不一致。缓解：单一行距函数、空白行/像素/bounds/命中回归。
- 风险：草稿自动提交意外改变 redo 或生成重复标注。缓解：以 revision、单事务和 undo/redo 测试锁定；只在非空文本时提交。
- 风险：文本重绘影响输入流畅度或绕过窗口策略。缓解：复用预览缓存、既有 Overlay 和节流，不添加窗口/worker，并运行源码守卫。
- 回滚：移除换行与自动提交边界，恢复既有单行文本输入；不影响标注数据、截图、贴图、导出、OCR、tray 或窗口策略。

## 完成记录

- 已完成：`AnnotateSession::text_insert_line_break` 只在文本草稿内插入 `\n`，不推进 revision。`draw_text`、无字体 fallback 和 `text_bounds` 共享行距并逐行处理；空白行仍占据 bounds，选择、移动和导出不再把多行压缩为单行。
- 已完成：Overlay 将 `Shift+Enter` 定义为换行，`Enter`/`Ctrl+Enter` 定义为提交，`Esc` 仍取消草稿。鼠标在选区外重选、或通过工具栏切换至其他工具时，非空文本会先通过既有单个 `AnnotationDoc` 事务提交；空白草稿清除后可继续当前外部点击。
- 已完成：没有新增窗口、事件循环、系统菜单、截图、worker 或可见性调用；文本继续使用既有 Overlay、预览缓存、资产 generation 和 `window_policy` tray-only 边界。
- 已验证：`cargo test -p pinora-core annotate -- --nocapture`、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`、`cargo test -p pinora-app window_policy::tests -- --nocapture`、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check` 与 `ctx validate` 通过；全量离线结果为 app 183 通过、2 忽略，core 85 通过。
- 未验证：离线测试无法证明真实 Windows、macOS、X11、KDE Wayland 中的输入法、字体、复杂 Unicode、HiDPI、输入帧时间、焦点、任务栏/Dock/分页器隔离或 tray 连续驻留；这些仍需原生会话验收。
