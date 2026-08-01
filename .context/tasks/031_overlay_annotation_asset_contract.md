# 任务 031：接入 Overlay 标注资产版本

- 状态：已完成
- 计划：`.context/plans/031_overlay_annotation_asset_contract.md`
- 规模：中
- 依赖：`.context/tasks/019_asset_generation_contract.md`、`.context/tasks/030_annotation_revision_contract.md`
- 生产行为变更：是；Overlay 标注或重选后，先前 OCR/导出任务的结果会被判为陈旧而不再交付。

## 目的

修复 `desktop_shell` 为 Overlay OCR 固定 `AssetRef::initial(image.id)`、又不随标注 revision 更新 current asset 的缺口，确保“标注事务版本”不是仅存在于 core 而实际任务路径忽略的元数据。

## 变更前记录

```text
目的：将 Overlay 选区合成图、标注 revision 与任务 AssetRef 连成可验证闭环。
影响路径：desktop_shell Overlay 状态、裁剪/合成、OCR/导出提交与轮询、离线回归测试、当前上下文文档。
兼容性：不改公共命令、持久化数据、状态字符串、租户或权限；已确认后关闭 Overlay 的复制/保存仍允许按冻结输入完成。
外部副作用：不访问真实桌面、OCR、剪贴板、网络或共享基础设施；只运行离线 Rust 测试与静态门禁。
回滚点：移除 Overlay 派生资产 helper 和调用点，恢复 030 前的 initial AssetRef 路径；保留 core revision 契约。
验证场景：有效 commit/undo、无效草稿、同尺寸重选、OCR/导出晚到、Overlay 已关闭的已确认导出。
```

## 任务目标

为已确认的 Overlay 选区创建独立的稳定 `ImageId`，每次生成合成图都使用该 ID；由 `AnnotationRevision.raw()` 构造当前 generation。将该 `AssetRef` 传入 OCR 和图像导出，轮询根据实时选区/revision 查询 current asset，保持已关闭 Overlay 的 pending 导出以其冻结引用完成。

## 影响路径

- `crates/pinora-app/src/desktop_shell.rs` 的 Overlay 状态、选区确认/重选、裁剪、任务提交与轮询。
- 必要的 `pinora-app` 离线测试；不改 OCR/导出服务协议。
- `.context/plans/031_overlay_annotation_asset_contract.md`、本任务、系统概览、风险登记和 `AGENTS.md`。

## 兼容性

- 接口：不新增对外命令；仅内部 Overlay 派生资产身份。
- 数据/状态：无持久化格式；`AssetRef` 只在进程内任务元数据中变化。
- 生命周期：关闭后的已确认导出继续使用 pending 的冻结资产；取消、再截和贴图关闭仍沿用既有 owner 关闭语义。
- 租户/权限：不涉及。

## 外部副作用

无。测试使用纯内存图像与 fake runner，不启动真实 OCR、系统剪贴板或桌面窗口。

## 回滚点

删除 Overlay 派生资产 helper、字段和调用点即可恢复原行为；不撤销 030 的领域 revision 或既有 JobSupervisor 门禁。

## 验证场景

- 同一选区的合成图 ID 与 `AssetRef.image_id` 一致，revision 直接映射 generation。
- 有效标注提交和 undo 后 current asset 改变；空 undo、无效或取消草稿保持不变。
- OCR/导出提交后编辑或同尺寸重选，晚到结果被标为 `StaleAsset`。
- Overlay 关闭后的已确认复制/保存仍能以 pending 冻结资产完成；用户取消/再截仍关闭 owner。

## 范围

- 建立 Overlay 选区派生资产 identity 与 revision 映射 helper。
- 迁移 Overlay OCR、复制、保存的图像和 current asset 查询。
- 补充离线回归测试及上下文完成记录。

## 非目标

- 不做 redo、标注持久化、文字层选择、设置、历史、跨平台 adapter 或 GUI E2E。
- 不修改 Pin 窗口的现有 asset 语义，也不实现贴图标注回编辑。
- 不更换 OCR/导出 runner 或改变剪贴板失败的产品语义。

## 预期文件

- `crates/pinora-app/src/desktop_shell.rs`。
- 必要时聚焦的内部 helper 模块与其测试。
- `.context/plans/031_overlay_annotation_asset_contract.md`、本任务。
- `.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- Overlay OCR/图像导出将选区+revision 资产同时冻结到 `JobSpec` 与输入图像。
- 有效编辑/重选使 current `AssetRef` 改变，现有监督器拒绝陈旧结果；无效操作不虚增版本。
- 已关闭但已确认的 Overlay 导出不被错误取消；全部离线质量门禁通过。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app ocr_job::tests -- --nocapture`
- `cargo test -p pinora-app export_job::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：复用裁剪时临时创建的 image ID，导致 JobSpec 与 ExportJobInput 失配。缓解：由 Overlay 状态拥有唯一选区 identity，合成图输出统一覆写为该 ID，并以单测锁定。
- 风险：重选同尺寸却未更新 ID。缓解：比较 source rect，任何变化都生成新选区 identity；不以尺寸作为身份判断。
- 风险：关闭 Overlay 后误丢弃确认导出。缓解：保留既有 pending 映射的冻结资产兜底，只让仍活跃 Overlay 使用实时引用。
- 回滚：删除本任务新增 helper 与接入，不影响 revision、任务服务和原子 PNG 基础。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：`overlay_ocr` 每次对临时裁剪图创建 `AssetRef::initial` 并保存为 `ocr_asset`；标注 `commit`/`undo` 只影响绘制缓存，`poll_ocr_jobs`/`poll_export_jobs` 不查询 `AnnotationDoc.revision`。因此有效编辑后的 OCR 或导出结果仍可能被接受。
- 实际变更：新增 Overlay 选区派生资产 identity；确认选区后每次裁剪/合成都使用同一个 `ImageId`，revision 无损映射 generation。有效 commit、非空 undo 与重选均使实时 current asset 改变；空 undo、无效或取消草稿不改变 revision/generation。
- 实际变更：OCR、复制和保存提交前冻结有效草稿；OCR/导出轮询使用实时 Overlay asset，已确认后关闭 Overlay 的复制/保存继续回退到 pending 冻结 asset。重选拖动一旦越过阈值立即更换 identity，使旧任务结果归类为陈旧而非被接受。
- 验证：`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture` 通过 5/5；OCR 服务 7/7、导出服务 7/7；`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`git diff --check` 与上下文校验均通过。workspace 为 app 76 项通过、2 项真实桌面测试忽略，core 42 项通过。
- 未覆盖项：未运行真实桌面、OCR 或系统剪贴板探针；不构成 GUI E2E。贴图未支持编辑标注，redo、标注持久化、历史、设置和跨平台适配仍不在本任务范围内。
