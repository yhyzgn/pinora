# 任务 030：建立标注 revision 领域契约

- 状态：已完成
- 计划：`.context/plans/030_annotation_revision_contract.md`
- 规模：中
- 依赖：`.context/tasks/019_asset_generation_contract.md`
- 生产行为变更：无；新增领域版本值和 mutation 语义，尚未改变桌面 UI 的导出/OCR 路径。

## 目的

落实设计文档“每次标注提交产生单调 revision”的前置契约，防止后续缓存、OCR 和导出只能依赖 `PinId` 或可变标注文档而误用旧结果。

## 任务目标

新增 `AnnotationRevision`，`AnnotationDoc` 保存私有 items 与当前 revision，暴露只读查询；有效 push/undo 推进 revision，`AnnotateSession::commit` 仅在真实追加标注时推进。revision 到最大值时饱和保持最大值，绝不回绕。

## 影响路径

- `crates/pinora-core/src/annotate.rs`、`crates/pinora-core/src/lib.rs`。
- 使用 `AnnotationDoc` 项目迭代的渲染函数和测试。
- 当前计划、任务、系统概览和风险登记。

## 兼容性

- 接口：新增 revision 查询；将可变 `items` 字段收敛为只读访问器，内部渲染和测试同步迁移。
- 数据/状态：当前无持久化标注文档；不改截图、OCR 文本、领域状态字符串、租户或权限。
- 生命周期：纯领域逻辑，无线程、文件、桌面或外部服务副作用。

## 外部副作用

无。只运行离线 Rust 单元测试和静态质量门禁。

## 回滚点

删除 revision 类型、私有字段收敛和相应测试；不影响已提交的任务监督或文件生命周期。

## 验证场景

- 新文档 revision 非零；push 后递增；成功 undo 后继续递增。
- 无项目时 undo、无效几何和空文本 commit 不改变 revision。
- 迭代器/切片仍允许渲染标注，源图像不被修改。

## 范围

- 新增 revision 值对象、AnnotationDoc 查询/变更语义和测试。
- 迁移 core 内所有 items 读取调用点。
- 更新上下文事实与后续 UI 接入风险。

## 非目标

- 不接入 desktop_shell、AssetRef、OcrJobService 或 ExportJobService。
- 不做 redo、持久化、历史栈、缓存键、跨平台或 GUI E2E。

## 预期文件

- `crates/pinora-core/src/annotate.rs`、`crates/pinora-core/src/lib.rs`。
- `.context/plans/030_annotation_revision_contract.md`、`.context/tasks/030_annotation_revision_contract.md`。
- `.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- revision 只在有效 mutation 后单调推进，永不回绕。
- 文档 items 无法被外部可变访问，既有渲染可使用只读访问器。
- 全部约定门禁通过，未接入 UI 的限制明确记录。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-core annotate::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：items 私有化遗漏渲染调用点。缓解：全局引用搜索与 workspace 编译。
- 风险：revision 最大值语义不明确。缓解：使用非零 `u64` 并在最大值饱和，绝不回绕；单测锁定。
- 回滚：撤销 annotate/core 公共导出改动；保留任务与资产基础设施。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：`AnnotationDoc` 只有公开 `items: Vec<Annotation>`，`push` 与 `undo` 没有版本信息；Overlay 的渲染/OCR/导出只能重新裁切可变文档，无法表达标注事务版本。
- 实际变更：新增 `AnnotationRevision`（非零、单调、最大值饱和）；`AnnotationDoc` 保存私有 `items` 与 revision，提供 `items()` 和 `revision()`。有效 `push`、非空 `undo` 会推进 revision；无效几何、空文本、取消草稿和空 undo 不推进。`bake_annotations` 改为读取只读切片。
- 验证：`cargo test -p pinora-core annotate::tests -- --nocapture` 通过 9 项；`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`git diff --check` 与上下文校验均通过。workspace 测试为 app 74 项通过、2 项真实桌面测试忽略，core 42 项通过。
- 未覆盖项：没有接入 `desktop_shell`、`AssetRef`、`OcrJobService` 或 `ExportJobService`；因此尚未证明真实 Overlay 编辑会拒绝陈旧 OCR/导出结果，也不构成 GUI E2E。
