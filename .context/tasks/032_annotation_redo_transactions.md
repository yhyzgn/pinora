# 任务 032：实现标注 redo 事务

- 状态：已完成
- 计划：`.context/plans/032_annotation_redo_transactions.md`
- 规模：中
- 依赖：`.context/tasks/030_annotation_revision_contract.md`、`.context/tasks/031_overlay_annotation_asset_contract.md`
- 生产行为变更：是；Overlay 支持 `Ctrl+Shift+Z` 和 `Ctrl+Y` 重做，且 redo 更新任务资产 generation。

## 目的

补齐设计文档声明但当前实现缺失的 redo，避免 undo 后只能重新绘制；同时确保恢复的标注仍作为新的可观察文档版本，不能让旧 OCR/导出结果覆盖重做后的合成图。

## 变更前记录

```text
目的：为单条已提交 Annotation 提供受版本保护的内存 redo 事务。
影响路径：pinora-core AnnotationDoc/测试、desktop_shell Overlay 快捷键、当前上下文文档。
兼容性：不改公共命令、持久化数据、稳定状态字符串、租户或权限；仅增加公开 redo 查询/操作。
外部副作用：无；只运行离线 Rust 测试与静态门禁，不启动真实桌面、OCR、剪贴板或网络服务。
回滚点：删除 redo 栈、redo API 和快捷键分支；保留 030 revision 与 031 Overlay asset 门禁。
验证场景：多次 undo/redo 顺序、新提交截断 redo、空操作、revision/AssetRef 变化、快捷键映射。
```

## 任务目标

`AnnotationDoc` 保存私有 redo 栈，提供 `redo()` 与 `can_redo()`；成功 undo/redo 都推进 revision，新的 `push` 清空 redo。Overlay 的 `Ctrl+Shift+Z` 和 `Ctrl+Y` 调用 redo，只有成功 mutation 才标记标注缓存脏并触发重绘。

## 影响路径

- `crates/pinora-core/src/annotate.rs` 的文档 mutation、查询和单元测试。
- `crates/pinora-app/src/desktop_shell.rs` 的 Overlay 键盘处理与离线测试。
- 当前计划、任务、系统概览、风险登记和 `AGENTS.md`。

## 兼容性

- 接口：新增 `AnnotationDoc::redo`/`can_redo`；不改变既有 `push`、`undo` 返回类型或 items 只读边界。
- 数据/状态：redo 仅进程内，不引入持久化格式；revision 继续非零、单调、饱和。
- 生命周期：不新增线程、子进程、文件或平台调用；031 的 current asset 查询自动消费新 revision。
- 租户/权限：不涉及。

## 外部副作用

无。测试仅使用内存中的图像和标注类型。

## 回滚点

移除 redo 栈、API、快捷键与测试，不影响既有 undo、栅格化或受监督任务服务。

## 验证场景

- 两条标注连续 undo 后按 LIFO redo 恢复，`items()` 顺序及栅格化输出可复现。
- undo 后新的有效 push 清空 redo；空 undo/redo 和无效草稿不推进 revision。
- redo 的 revision 改变，使同一 Overlay identity 生成不同 `AssetRef` generation。
- `Ctrl+Shift+Z` 与 `Ctrl+Y` 映射 redo，空 redo 不触发不必要重绘。

## 范围

- `AnnotationDoc` redo 存储、API、revision 语义与 core 测试。
- Overlay redo 快捷键和最小离线映射测试。
- 上下文完成记录。

## 非目标

- 不做持久化事务、跨对象批处理、选择/移动编辑、清空标注、贴图回编辑或 GUI E2E。
- 不修改任务监督器、OCR/导出输入结构、截图后端或系统剪贴板。

## 预期文件

- `crates/pinora-core/src/annotate.rs`。
- `crates/pinora-app/src/desktop_shell.rs`。
- `.context/plans/032_annotation_redo_transactions.md`、本任务。
- `.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- redo 契约保持顺序、清空分支和 revision 语义；内存集合不可被外部直接修改。
- Overlay 快捷键正确调用 redo，成功 redo 使当前 `AssetRef` 改变。
- 完整离线门禁通过，真实桌面/可访问性验证缺口明确记录。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-core annotate::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：新 push 后仍允许旧 redo，导致分叉历史错误。缓解：在 `push` 成功前清空 redo，单测锁定。
- 风险：redo 复用 push 而清空后续 redo。缓解：单独内部恢复路径，连续 redo 测试覆盖。
- 风险：空快捷键仍触发缓存重绘。缓解：依据 mutation 返回值设置 `annotate_dirty`。
- 回滚：删除 redo 分支和新增 API；保留版本契约与任务门禁。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：`AnnotationDoc::undo` 直接丢弃最后项目，缺少 redo 栈；Overlay 只有 `Ctrl+Z` 分支，设计文档的 redo 事务与 revision 一致性尚未落地。
- 实际变更：`AnnotationDoc` 增加私有 redo 栈、`redo()` 与 `can_redo()`；undo 和 redo 均按 LIFO 迁移同一 Annotation 并推进 revision。新有效 `push` 清空 redo，空历史操作不改变 revision。
- 实际变更：Overlay 抽出可离线测试的历史快捷键解析；`Ctrl+Z` 为 undo，`Ctrl+Shift+Z` 和 `Ctrl+Y` 为 redo。仅成功 undo/redo 刷新标注缓存；redo 后产生的新 revision 被 031 的 AssetRef 映射识别为新 generation。
- 验证：`cargo test -p pinora-core annotate::tests -- --nocapture` 通过 11/11；`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture` 通过 7/7；`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`git diff --check` 与上下文校验均通过。workspace 为 app 78 项通过、2 项真实桌面测试忽略，core 44 项通过。
- 未覆盖项：未运行真实桌面快捷键、OCR 或系统剪贴板探针；不构成 GUI E2E。redo 仅在内存中保存单条标注事务，持久化、批量事务、清空与贴图编辑仍待后续任务。
