# 计划 019：资产版本与陈旧结果拒绝契约

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/019_asset_generation_contract.md`

## 目标

落实生产重构设计的第一条可执行领域契约：为截图资产引入稳定的 ID 与 generation，并让后续 OCR、导出、渲染和窗口工作流能够判定任务结果是否仍对应当前资产版本。

## 非目标

- 不重写 `desktop_shell`、替换截图后端或实现真实 OCR 取消。
- 不修改现有 `CaptureImage` 的公共字段、持久化形状或用户可观察行为。
- 不新增平台 SDK、后台线程或外部进程。

## 约束

- 新类型必须位于 `pinora-core`，只依赖领域类型和标准库。
- generation 只能单调递增，比较语义必须有单元测试。
- 采用增量兼容方式；旧路径未迁移前不得删除或强制使用新类型。
- 不使用 warning suppression。

## 依赖关系

- 依赖 016 的“保留 core 领域不变量、重做应用/平台边界”结论。
- 依赖 017 的 fmt/Clippy/test 质量基线。
- 依赖 018 中的 `CaptureAsset`、generation 和陈旧结果拒绝设计契约。

## 阶段

1. 盘点现有 ID、图像与事件类型，确认新增模块及导出边界。
2. 定义纯领域资产引用和 generation 比较类型，先补单元测试。
3. 实现最小模块、更新公共导出并运行全量质量门禁。

## 检查点

- `AssetRef` 必须能同时表达资产 ID 与 generation，且构造/推进不产生无效版本。
- 陈旧结果检查只能依据同一资产和 generation 比较，不偷用 UI 窗口句柄或时间戳。
- 所有现有 workspace 测试保持通过。

## 计划级风险

- 过早把新类型接入所有遗留路径会放大 `desktop_shell` 重构范围；本任务只建立并测试领域契约。
- 现有 `ImageId` 可能有既定导出语义；新增类型必须复用而不是替换它，后续迁移逐点进行。

## 完成标准

- `pinora-core` 提供经测试的资产版本引用与陈旧结果判断接口。
- 现有领域模型和工作流无行为回归。
- fmt、check、严格 Clippy、workspace 测试和上下文校验通过。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：新增 `pinora-core::asset`，提供 `AssetGeneration`（非零、不回绕）和由既有 `ImageId` 组合的 `AssetRef`；`accepts_result` 只在资产 ID 与 generation 完全相等时接受结果。
- 验证：3 个定向资产契约测试通过；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、workspace 测试、`git diff --check` 和上下文校验通过。
- 迁移状态：旧 UI、OCR、导出和截图缓存尚未接入该契约，仍须在后续垂直切片按 owner/generation 迁移。
