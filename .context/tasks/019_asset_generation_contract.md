# 任务 019：建立资产 generation 领域契约

- 状态：已完成
- 计划：`.context/plans/019_asset_generation_contract.md`
- 规模：小
- 依赖：`.context/tasks/018_production_design_rebuild.md`
- 生产行为变更：无

## 目的

在 `pinora-core` 中新增兼容的资产版本引用值对象，使应用工作流可以在不依赖 UI 或平台句柄的前提下拒绝陈旧的异步结果。

## 任务目标

新增 `AssetGeneration` 和 `AssetRef`：generation 非零且不回绕，引用精确组合既有 `ImageId` 与 generation，并只接受同一资产同一版本的任务结果。

## 影响路径

- `crates/pinora-core/src/` 的 ID/图像领域模块和公共导出。
- `crates/pinora-core` 的定向单元测试。
- 当前计划、任务和系统概览。

## 兼容性

- 接口：只新增领域类型与查询方法，不移除或改写现有导出符号。
- 数据：无持久化格式；值对象只在内存中使用。
- 状态：不改变现有状态字符串或截图/贴图工作流。
- 租户/权限：不涉及。

## 外部副作用

无；只运行离线 Rust 构建和测试，不启动图形桌面或外部服务。

## 回滚点

反转新增领域模块、导出和测试即可；既有 `CaptureImage` 与应用行为不受影响。

## 验证场景

- generation 从初始值推进并保持单调性。
- 同资产且 generation 相同的结果可接受；较旧 generation 与不同资产均被拒绝。
- 现有 core/app 测试继续通过。

## 范围

- 查找 `ImageId`、`CaptureImage` 和现有 ID 导出引用。
- 新增资产版本引用及陈旧结果判断的纯领域实现和测试。
- 更新必要的 `pinora-core` 公共导出与上下文完成记录。

## 非目标

- 不接入 OCR、导出、截图缓存或窗口运行时。
- 不改变图像像素、标注模型、命令字符串或平台能力。
- 不创建后台任务/进程实现。

## 预期文件

- `crates/pinora-core/src/asset.rs` 或与现有 ID 模块一致的聚焦模块。
- `crates/pinora-core/src/lib.rs`。
- 当前计划、任务与 `.context/system/overview.md`。

## 验收标准

- 新增 API 可表达 `ImageId + generation` 并有明确的陈旧结果判断。
- 核心模块不依赖 UI、平台或外部进程。
- 不引入 warning suppression；所有约定质量门禁通过。

## 验证

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：新 ID 类型若与现有 `ImageId` 重叠，会增加迁移困惑。缓解：只组合既有 `ImageId`，不创建第二套图像 ID。
- 风险：过度接入遗留 UI 会扩大任务范围。缓解：仅交付纯领域模块和测试，集成留给后续垂直切片。
- 回滚：删除新增类型、导出和测试即可，不触碰现有业务流程。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：018 已定义 `CaptureAsset` generation 和任务陈旧结果拒绝契约；当前代码尚无该值对象，且 `desktop_shell` 仍直接承载 OCR/窗口生命周期。
- 实际变更：新增 `crates/pinora-core/src/asset.rs`，公开 `AssetGeneration` 与 `AssetRef`，并在 `lib.rs` 导出；不改动既有 `CaptureImage`、平台调用或用户可观察工作流。
- 验证：`cargo test -p pinora-core asset::tests -- --nocapture` 通过 3/3；`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（40 app + 38 core 通过，2 个真实桌面测试忽略）、`git diff --check` 和上下文校验均通过。
- 残留风险：该任务只建立领域契约，不能自行阻止遗留 `desktop_shell` 中的陈旧 OCR/剪贴板结果；集成需由后续任务完成。
