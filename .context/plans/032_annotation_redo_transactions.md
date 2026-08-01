# 计划 032：标注 redo 事务

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/032_annotation_redo_transactions.md`

## 目标

为 `AnnotationDoc` 建立内存内 redo 栈：每次有效提交是一项事务，undo 将最后事务移入 redo，redo 将其恢复到文档。每次有效 undo/redo 都推进 revision，使 031 的 Overlay 资产门禁自动拒绝重做前的陈旧 OCR/导出结果。

## 非目标

- 不实现跨会话持久化事务日志、批量事务、清空标注、对象选择/编辑或贴图回编辑。
- 不改变标注栅格化、OCR/导出服务、平台接口或持久化数据形状。
- 不声称离线输入处理测试等同于真实桌面 E2E。

## 约束

- 新的有效 `push` 必须清空 redo 分支；无效草稿、空 undo 和空 redo 不得改变 revision 或历史。
- redo 只能恢复最近 undo 的同一 `Annotation`，保持原始绘制顺序；不得通过 `push` 恢复而错误清空后续 redo。
- `items`、redo 栈继续保持私有，外部只能读取当前项目与查询 redo 可用性。
- Overlay 快捷键只调用领域 mutation，current asset 仍由 revision 实时映射，不能在 UI 中自行伪造 generation。

## 依赖关系

- 依赖 030 的私有标注集合和单调 revision。
- 依赖 031 的 Overlay revision -> `AssetRef` 映射与陈旧结果拒绝。

## 阶段

1. 在 core 中增加 redo 存储、查询与 push/undo/redo 语义，先补单元测试。
2. 接入 Overlay 的 `Ctrl+Shift+Z` 与 `Ctrl+Y`，保持无效操作不触发版本变化。
3. 运行领域、Overlay 和 workspace 门禁，记录未覆盖的桌面交互。

## 检查点

- 连续 undo/redo 保持 LIFO 顺序，渲染结果与未 undo 前一致。
- 在 undo 后新增标注会清空 redo，后续 redo 为空且 revision 不变。
- redo 后 Overlay current `AssetRef` generation 不等于 undo 前或 undo 后的旧版本。

## 计划级风险

- 当前每条 `Annotation` 被视为一个事务；未来复合编辑必须新增专用事务模型，不能悄悄复用单对象栈。
- `desktop_shell` 仍是单体，快捷键接入保持局部，不将 UI 历史状态复制到应用层。

## 完成标准

- core 提供可测试的 redo 契约，Overlay 支持标准快捷键并正确刷新缓存。
- redo/undo 均推进 revision，旧任务结果可被已有资产门禁拒绝。
- 约定的离线质量门禁通过，真实桌面验证缺口明确记录。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：`AnnotationDoc` 新增私有 redo 栈与 `redo()`/`can_redo()`；undo 将项目按 LIFO 顺序压入 redo，redo 不经过 `push` 而恢复项目，连续 redo 保持顺序。新的有效 `push` 清空 redo 分支；空 undo/redo 不变更 revision。
- 实际变更：有效 undo/redo 都推进 revision，Overlay 的 `Ctrl+Shift+Z` 与 `Ctrl+Y` 调用 redo；只有 mutation 成功才标记缓存脏和请求重绘，因此 031 的实时 asset 映射会把 redo 视为新 generation。
- 验证：标注领域测试 11/11、Overlay 纯逻辑测试 7/7；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（app 78 项通过、2 项真实桌面测试忽略；core 44 项通过）、差异检查与上下文校验通过。
- 残留风险：每条 Annotation 仍是一项内存事务，不支持持久化、跨对象批处理、清空、选择编辑或贴图回编辑；没有真实桌面快捷键 E2E。
