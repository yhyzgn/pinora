# 计划 020：截图能力真实语义

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/020_capture_capability_truth.md`

## 目标

移除运行时截图后端自动选择中的 fake 成功回退，使真实截图后端不可用时返回可诊断的能力失败，而非创建模拟截图并触发后续贴图、导出或 OCR 工作流。

## 非目标

- 不替换 KDE/xcap 截图技术、不承诺新平台支持。
- 不删除 `FakeCaptureProvider`；它仍是离线测试与显式注入的测试实现。
- 不实现完整 `CapabilitySnapshot` 或 UI 受限状态页。

## 约束

- 生产自动选择只能返回 KDE 或 xcap 实现；两者均不可用时不得构造 fake 图像。
- 错误必须保留两个后端的失败摘要，但不写入敏感环境数据。
- 测试显式构造 fake provider，不能再把自动选择 fake 当作正确行为。
- 保持启动/编排对受限能力的可恢复处理；不以 panic 或进程中止替代错误结果。

## 依赖关系

- 依赖 016 对 fake 回退风险的审计结论。
- 依赖 017 的静态质量基线和 019 的领域契约基础。

## 阶段

1. 盘点 `SelectedCaptureProvider` 的调用者、启动路径和测试。
2. 将自动探测 API 改为显式结果，调整应用层的失败处理与测试。
3. 验证不可用后端不会生成图像或发布成功路径。

## 检查点

- 生产 `autodetect` 不包含 `FakeCaptureProvider::new()`。
- fake 测试仍可通过显式依赖注入验证领域工作流。
- 所有公开签名变更先完成引用搜索并由 workspace 编译验证。

## 计划级风险

- 当前启动可能假定至少存在一个 provider；需要用明确错误传播或受限运行态保持可恢复，而不是隐式重建 fake。
- 本任务不能证明真实 KDE/xcap 捕获可用；那仍需授权桌面探针。

## 完成标准

- 自动截图选择没有 fake 成功回退，且无后端时产生受控错误。
- 对应测试覆盖 KDE、xcap 与无可用后端三种分支。
- fmt、check、严格 Clippy、workspace 测试和上下文校验通过。

## 完成记录

- 状态：已完成（2026-08-01）。
- 生产自动探测已从 KDE → xcap → fake 改为 KDE → xcap → `Unavailable`；受限 provider 的显示与截图调用均返回 `CapabilityUnavailable`，不生成模拟图像。
- `FakeCaptureProvider` 仍通过 `fake_only()` 显式注入，供离线测试和开发工作流使用。
- 质量门禁与上下文校验已通过；真实 KDE/xcap 桌面授权探针仍未执行，不能据此宣称真实捕获已生产验证。
