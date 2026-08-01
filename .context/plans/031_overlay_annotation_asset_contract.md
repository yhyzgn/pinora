# 计划 031：Overlay 标注资产版本接入

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/031_overlay_annotation_asset_contract.md`

## 目标

让每个 Overlay 已确认选区拥有稳定的派生资产身份，并把当前 `AnnotationDoc.revision` 映射为该资产的 `AssetRef.generation`。OCR 或导出提交后，只要选区、有效标注提交或撤销改变当前资产，晚到结果就必须被任务监督器拒绝。

## 非目标

- 不实现 redo、标注持久化、历史索引、渲染缓存淘汰或贴图回编辑。
- 不改变 OCR 引擎、导出格式、系统剪贴板、截图后端或领域状态字符串。
- 不把离线服务/状态测试称为 GUI E2E 或真实桌面探针。

## 约束

- 选区派生图在同一有效选区内保持同一个 `ImageId`；仅内容源或选区改变时更换身份。
- `AnnotationRevision` 的非零值必须无损映射为 `AssetGeneration`；不能借用时间戳、窗口句柄或可变 `PinId` 作为版本。
- 任务输入图像 ID 必须与提交的 `AssetRef.image_id` 一致；不能通过修改任务元数据绕过现有 `ExportJobService` 检查。
- 只改当前 Overlay 垂直路径；贴图尚无标注编辑能力，不伪造其已接入 revision。

## 依赖关系

- 依赖 019 的 `AssetRef`/generation 陈旧结果契约。
- 依赖 021、023、026 的任务监督和输入身份校验。
- 依赖 030 的单调 `AnnotationRevision` 与私有标注集合。

## 阶段

1. 为 Overlay 已确认选区建立可测试的派生资产身份与 revision 到 generation 映射。
2. 让 OCR、复制和保存使用冻结的当前合成图与该资产引用；轮询时查询实时 current asset。
3. 在有效标注提交、undo 和选区变化后推进或替换 current asset，并用纯逻辑测试锁定陈旧结果拒绝。

## 检查点

- 同一选区在未编辑时的 OCR/导出输入图像 ID 与 `AssetRef` 精确匹配。
- 有效 commit/undo 后 generation 改变；空 undo、无效或取消草稿不改变 generation。
- 选区改变即使尺寸相同也更换 image identity，旧 OCR/导出不能被当作当前结果。

## 计划级风险

- `desktop_shell` 是已知单体，接入应收敛在纯 helper 与少量调用点，不能顺带重写事件循环。
- Overlay 关闭后的已确认复制/保存仍可按原任务冻结资产完成；不能因移除 Overlay 而误判为陈旧。

## 完成标准

- Overlay 的 OCR/导出任务均引用选区+revision 资产，并由现有 `JobSupervisor` 拒绝编辑或重选后的晚到结果。
- 核心映射和 Overlay helper 有离线回归测试；所有约定质量门禁通过。
- 真实桌面验证、redo 与贴图编辑等未交付项明确记录。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：Overlay 确认选区持有独立 `OverlayAssetIdentity`；该身份稳定覆盖每次派生合成图的 `ImageId`，`AnnotationRevision.raw()` 映射为 current `AssetRef.generation`。有效标注提交、非空 undo 和同尺寸重选都会使 current asset 改变；重选阈值触发时立即更换身份，避免过渡期间接受旧任务结果。
- 实际变更：Overlay OCR、复制和保存均冻结同一合成图与 `AssetRef`；轮询改为读取实时选区/revision。执行 OCR/导出前会提交有效草稿，避免预览像素与任务版本不一致。已确认后关闭 Overlay 的导出继续用 pending 映射中的冻结引用完成。
- 验证：Overlay 纯逻辑测试 5/5、OCR 服务 7/7、导出服务 7/7；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（app 76 项通过、2 项真实桌面测试忽略；core 42 项通过）、差异检查与上下文校验通过。
- 残留风险：没有真实窗口 E2E；贴图尚无标注回编辑，因此只使用其不可变快照的 initial generation；redo、标注持久化和渲染缓存键仍待独立任务。
