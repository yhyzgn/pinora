# 任务 069：Overlay 标注整体清空与可撤销事务

- 状态：已完成
- 计划：`.context/plans/069_clear_annotation_transaction.md`
- 规模：中
- 依赖：`.context/tasks/030_annotation_revision_contract.md`、`.context/tasks/032_annotation_redo_transactions.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/066_auxiliary_window_visibility_policy.md`
- 生产行为变更：是；当前 Overlay 可整体清空标注并通过既有撤销/重做恢复。

## 任务目标

为当前 Overlay 提供安全的“清空标注”工具栏动作，并将其实现为单一 `AnnotationDoc` 事务。用户可一次撤销完整恢复、一次重做再次清空；行为与标注 revision、预览缓存、导出和贴图 generation 门禁兼容，且 Pinora 始终只由 tray 驻留。

## 范围

- 把 `AnnotationDoc` 的内部历史扩展为单项新增与整体清空事务。
- 提供空清空无副作用的领域 API，并接入现有 Overlay 工具栏。
- 增加事务顺序、revision、redo 分支、工具栏布局/命中、Overlay/window policy 回归。
- 更新计划、任务、系统事实和风险记录。

## 非目标

- 不实现对象选择/局部删除、确认弹窗、跨会话持久化、贴图内编辑、截图、窗口或任务改造。

## 预期文件

- `crates/pinora-core/src/annotate.rs`
- `crates/pinora-app/src/{desktop_shell.rs,overlay_toolbar.rs,window_policy.rs}`
- `AGENTS.md`
- `.context/plans/069_clear_annotation_transaction.md`
- `.context/tasks/069_clear_annotation_transaction.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 多条标注清空后一次 undo 按原绘制顺序恢复，一次 redo 再次清空；空清空不推进 revision。
2. 清空后新增标注清除 redo 分支，既有单条撤销/重做顺序不回归。
3. 工具栏入口命中、窄画布布局和 Overlay 缓存失效正确；无新窗口、事件循环、截图或 worker。
4. `window_policy` 递归源码守卫及全量离线门禁通过；真实桌面交互、HiDPI、tray 和任务栏/Dock 仍明确未验证。
5. Pinora 空闲时仅以系统 tray 常驻；任何 Overlay、贴图或辅助层均不得产生任务栏、Dock 或分页器项。该要求由唯一窗口工厂和展示入口约束，仍需四类原生桌面实机验收。

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

- 风险：清空事务持有对象快照。缓解：仅在有对象时生成快照，随后新编辑即清除 redo。
- 风险：事务栈重构破坏既有 redo。缓解：锁定单条和清空交错顺序、revision 和 redo 分支测试。
- 风险：入口绕过窗口策略。缓解：只扩展现有工具栏/Overlay 状态机并运行递归守卫。
- 回滚：移除清空动作与事务类型；保留原有标注、tray 和窗口策略。

## 完成记录

- 已完成：`AnnotationDoc` 以新增/整体清空事务维护历史；清空后一次 undo 恢复全部原标注与顺序，一次 redo 重新清空，空文档清空无副作用，清空后新标注清除 redo 分支。
- 已完成：工具栏“清空”命中和状态机接入；清空同时丢弃未提交草稿，仅标记当前 Overlay 重绘，不建立窗口、后台任务、截图或系统菜单。
- 已验证：`cargo test -p pinora-core annotate -- --nocapture`、`cargo test -p pinora-app overlay_toolbar::tests -- --nocapture`、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`、`cargo test -p pinora-app window_policy::tests -- --nocapture`、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check` 与 `ctx validate` 均通过；全量离线结果为 app 173、core 75，另有 2 个真实桌面测试忽略。
- 未验证：真实 Windows、macOS、X11、KDE Wayland 中 Overlay、贴图和任何辅助窗口均不会出现在任务栏、Dock 或分页器；也未测量高 DPI、大文档清空和连续绘制的输入/呈现延迟。
