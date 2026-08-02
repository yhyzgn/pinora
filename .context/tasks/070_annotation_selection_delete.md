# 任务 070：Overlay 标注选择、删除与可撤销恢复

- 状态：已完成
- 计划：`.context/plans/070_annotation_selection_delete.md`
- 规模：中
- 依赖：`.context/tasks/030_annotation_revision_contract.md`、`.context/tasks/032_annotation_redo_transactions.md`、`.context/tasks/068_overlay_preview_cache.md`、`.context/tasks/069_clear_annotation_transaction.md`
- 生产行为变更：是；当前 Overlay 支持选择最上层标注并删除/撤销恢复。

## 任务目标

让用户在既有 Overlay 内点击选择一条标注，看到不写入导出图像的高亮，并以 Delete/Backspace 删除；删除与既有 undo/redo、清空、预览缓存及资产 generation 完整协作，Pinora 仍只以 tray 常驻。

## 范围

- 为 `AnnotationDoc` 增加确定性顶层命中与单对象删除事务。
- 为 Overlay 增加选择工具、工具栏入口、视觉高亮和 Delete/Backspace。
- 在每个文档变更或 Overlay 生命周期变化时使选择失效。
- 覆盖事务、几何命中、预览/导出隔离、工具栏/键盘和窗口策略回归，并更新上下文事实和风险。

## 非目标

- 不实现拖动、缩放、旋转、样式编辑、多选、框选、排序、跨文档复制粘贴、持久化编辑历史或贴图内独立对象编辑。
- 不改捕获、贴图、OCR、导出、历史、平台窗口策略、系统菜单或后台任务。

## 预期文件

- `crates/pinora-core/src/annotate.rs`
- `crates/pinora-app/src/{desktop_shell.rs,overlay_toolbar.rs}`
- `AGENTS.md`
- `.context/plans/070_annotation_selection_delete.md`
- `.context/tasks/070_annotation_selection_delete.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 点击重叠对象只选择最上层；矩形、圆角矩形、椭圆、线、箭头、画笔、序号、文本、马赛克和模糊均有确定性命中，空点击不修改文档或 revision。
2. Delete/Backspace 删除选中项；一次 undo 原位置恢复，一次 redo 再次删除；新标注清除 redo 分支，无选中删除无副作用。
3. 选中高亮不进入 `AnnotationDoc`、导出、贴图或 OCR 输入；文档变更、重选、清空、取消和 Overlay 关闭均清除选择。
4. 不创建窗口、事件循环、系统菜单、截图或 worker；Pinora 空闲仅在 tray，Overlay/贴图/辅助层禁止出现任务栏、Dock 或分页器项，继续由 `window_policy` 递归守卫约束。

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

- 风险：对象命中几何与视觉描边不完全一致。缓解：所有类型测试、最上层优先、固定像素容差；真实 HiDPI 点击另行验收。
- 风险：删除事务索引错误破坏重做顺序。缓解：用删除位置和对象快照形成私有事务，覆盖与清空/新增交错。
- 风险：高亮污染导出或绕过窗口策略。缓解：仅作为现有 Overlay chrome 绘制，不改变提交层或建窗路径，并运行递归守卫。
- 回滚：移除选择工具、高亮和删除事务；保留已有新增、撤销/重做、清空、缓存、tray 与窗口策略。

## 完成记录

- 已完成：新增 `Select` 工具、工具栏入口和 `V` 快捷键；点击按反向绘制顺序选择最上层已提交标注，空点击只清除瞬态选择。
- 已完成：新增按索引删除事务，Delete/Backspace 删除选中项；undo 原位恢复、redo 再次删除，与清空和后续新增保持确定性顺序及 redo 分支语义。
- 已完成：选中框仅绘制在既有 Overlay 的 XRGB 呈现帧；文档提交、undo/redo、清空、重选、切换工具和 Overlay 生命周期变化都会清除选择，因此不污染导出、贴图、OCR 或缓存键。
- 已验证：`cargo test -p pinora-core annotate -- --nocapture`、`cargo test -p pinora-app overlay_toolbar::tests -- --nocapture`、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 与 `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace` 通过；全量离线结果为 app 173 通过、core 78 通过，另有 2 项真实桌面测试忽略。
- 未验证：真实 Windows、macOS、X11、KDE Wayland 的选择命中、高亮、Delete/Backspace、HiDPI、输入/呈现延迟，以及 Overlay、贴图和辅助层绝不进入任务栏、Dock 或分页器的窗口管理器结果。
