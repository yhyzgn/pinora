# 任务 004：实现 pinora-core 领域数据模型

- 状态：已完成
- 计划：`.context/plans/004_domain_models.md`
- 规模：中
- 依赖：`.context/tasks/003_app_runtime_core.md`
- 生产行为变更：无（纯库类型；main 行为不变）

## 任务目标

在 `pinora-core` 增加几何、`CaptureImage`、`Pin`/`PinTransform` 与 `AppState` 贴图集合的最小实现，并配单元测试。

## 范围

- 新增模块：`geometry`、`image`、`pin`（或等价拆分）。
- 扩展 `AppState`：持有 `Vec<Pin>` 与创建/关闭贴图的纯逻辑方法。
- 更新 `lib.rs` 导出与 `.context` 指针。

## 非目标

- 真实像素缓冲编解码、窗口句柄、平台坐标转换。
- 标注/OCR 完整模型（可预留空结构或后续任务）。

## 预期文件

- `crates/pinora-core/src/**`
- `.context/plans/004_*.md`、`.context/tasks/004_*.md`、`AGENTS.md`、system 文档

## 验收标准

- 可构造 `CaptureImage` 与 `Pin`，变换字段可更新。
- `AppState` 能添加/关闭 pin，关闭不存在 pin 时返回明确错误。
- `cargo test --workspace` 全部通过。

## 验证

- `cargo test --workspace`
- `cargo check --workspace`
- `cargo run`（行为与 003 一致）

## 风险与回滚

- 风险：字段与未来平台 DPI 语义不完全匹配。缓解：注释 + 可演进结构。
- 回滚：删除新增模块，恢复 `state.rs`。

## 完成记录

- 状态：已完成（2026-07-30）。
- 实际变更：新增 `geometry`/`image`/`pin` 模块；扩展 `ImageId`/`PinId`、`AppState` 贴图集合与 create/close/transform；错误码 `NotFound`。
- 实际验证：`cargo test --workspace` 26 passed；`cargo run` 行为未回归。
- 未解决项：截图/贴图命令分发与真实像素捕获。
