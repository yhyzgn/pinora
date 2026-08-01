# 计划 030：标注 revision 领域契约

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/030_annotation_revision_contract.md`

## 目标

为 `AnnotationDoc` 建立单调、不可回绕的 revision 值对象；每次有效标注提交或撤销都会推进 revision，渲染缓存、OCR 和导出可在后续任务中显式冻结该版本，避免编辑后的陈旧合成结果复用。

## 非目标

- 不在本计划中实现 redo、标注持久化、贴图回编、渲染缓存键或 UI 资产 generation 接入。
- 不重写标注工具、栅格化算法、事件协议或 OCR/导出服务。
- 不把领域单测称为桌面标注 E2E。

## 约束

- revision 属于 `pinora-core`，不得持有窗口、线程、路径或平台类型。
- 有效 push/undo 必须单调递增且不回绕；无效草稿提交和空 undo 不得推进 revision。
- 标注文档的项目集合不允许绕过 mutation API 直接被外部修改。
- 公共导出变更前需搜索全部引用并保持现有标注渲染行为。

## 依赖关系

- 依赖 019 的资产 generation 设计原则；本任务只建立独立 annotation revision。
- 后续任务将把 revision 映射到 Overlay 合成图和 OCR/导出 `AssetRef`。

## 阶段

1. 新增 revision 值对象与 AnnotationDoc 查询接口，封闭可变 items。
2. 修改 push/undo 和 AnnotateSession 提交路径的 revision 迁移。
3. 增加单元测试并运行全量门禁。

## 检查点

- 新文档 revision 从非零初始值开始，成功提交或 undo 后严格增加。
- 无效几何、空文本、取消草稿或空 undo 不改变 revision。
- 渲染仍读取只读项目切片，不依赖公开 Vec 字段。

## 计划级风险

- 现有 Overlay 尚未把 revision 纳入 AssetRef；本任务不能单独阻止 UI 层陈旧结果，后续必须接入。
- `u64` 最大值不可在实践中到达，但 API 必须不产生回绕值。

## 完成标准

- `pinora-core` 暴露可测试的标注 revision，AnnotationDoc mutation 遵守单调语义。
- 既有标注渲染与会话测试保持通过，新增 revision 边界测试通过。
- fmt、check、严格 Clippy、workspace 测试、差异检查和上下文校验通过。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：新增基于非零 `u64` 的 `AnnotationRevision`；新文档从 1 开始，有效 `push` 和非空 `undo` 饱和递增，最大值保持最大值。`AnnotationDoc.items` 改为私有，只允许通过只读切片查询，栅格化路径同步使用该访问器。
- 验证：标注领域测试 9 项通过；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`、差异检查和上下文校验均通过（app 74 项通过、2 个真实桌面测试忽略；core 42 项通过）。
- 残留风险：当前 Overlay 的合成资产、OCR 和导出输入尚未携带 annotation revision；后续必须将 revision 映射至 `AssetRef` generation，才可在 UI 路径拒绝编辑后的陈旧结果。
