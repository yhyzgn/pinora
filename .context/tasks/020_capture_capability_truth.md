# 任务 020：禁止运行时 fake 截图成功回退

- 状态：进行中
- 计划：`.context/plans/020_capture_capability_truth.md`
- 规模：中
- 依赖：`.context/tasks/019_asset_generation_contract.md`
- 生产行为变更：是；无真实截图能力时不再继续生成模拟图像。

## 目的

让自动截图选择准确反映系统能力：KDE 和 xcap 均不可用时返回可诊断错误；`FakeCaptureProvider` 只可由测试或显式开发注入使用。

## 任务目标

将生产自动探测从“KDE → xcap → fake 成功回退”改为“KDE → xcap → 受限能力状态”，并让受限状态的截图与显示调用返回 `CapabilityUnavailable`，同时保留显式 fake 注入用于离线契约测试。

## 影响路径

- `crates/pinora-app/src/capture_select.rs`。
- 选择器的调用者、应用启动路径与定向测试。
- 当前计划、任务、系统概览和风险登记。

## 兼容性

- 接口：自动探测 API 可能从直接 provider 改为 `Result`，所有引用必须同步。
- 数据/状态：不改持久化或稳定领域状态字符串；改变无后端环境下的运行时结果，从 fake 成功改为可恢复失败。
- 租户/权限：不涉及；不会请求真实桌面权限。

## 外部副作用

无。只运行离线单元测试与构建，不启动图形桌面或共享外部服务。

## 回滚点

反转选择器、调用方和测试改动可恢复原行为；不改变真实截图后端实现。

## 验证场景

- KDE provider 可用时自动选择 KDE。
- KDE 不可用但 xcap 可用时选择 xcap。
- 两者不可用时自动选择返回错误，且不构造 fake 图像。
- 显式 fake 注入的现有测试仍通过。

## 范围

- 修改自动选择器及必要调用方、测试。
- 更新系统事实和 fake 回退风险状态。

## 非目标

- 不替换或验证 KDE/xcap 后端，不实现新的截图 SDK。
- 不实现完整受限 UI、平台能力页面或跨平台 adapter。
- 不删除 fake 测试 provider。

## 预期文件

- `crates/pinora-app/src/capture_select.rs`。
- 其必要调用方和测试。
- 当前计划、任务、`.context/system/overview.md`、`.context/system/risks.md`。

## 验收标准

- 运行时自动选择器不会返回 fake provider。
- 无真实后端时返回可诊断错误，错误信息含后端失败摘要。
- fake 保持为显式测试实现；所有质量门禁通过。

## 验证

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：无截图后端的开发环境不再获得模拟 Overlay。缓解：测试显式使用 fake；运行时显示后端不可用的可诊断错误。
- 风险：调用方把 provider 视为必定存在。缓解：先搜索所有引用，使用 `Result` 显式传播或受限状态处理。
- 回滚：反转本任务的选择器与调用方改动；不影响截图后端或领域模型。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：`capture_select.rs` 在 KDE/xcap 均不可用时构造 `FakeCaptureProvider`，审计确认这会把模拟图像误报为截图成功。
- 实际变更：生产 `autodetect` 现在只选择 KDE 或 xcap；两者均不可用时返回 `SelectedCaptureProvider::Unavailable`，保留两个后端的诊断摘要，并由 `CaptureProvider` 返回 `ErrorCode::CapabilityUnavailable`。`fake_only()` 仍是显式测试注入入口。
- 验证：`cargo fmt --check`、`cargo test -p pinora-app capture_select::tests -- --nocapture`（4/4）、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（43 app + 38 core 通过，2 个真实桌面测试忽略）、`git diff --check` 均通过。
- 上下文校验：`context_bootstrap.py validate` 在补齐本任务 `## 任务目标` 后通过。
- 残留风险：本任务没有证明 KDE/xcap 在真实桌面授权下可用；受限状态目前提供能力错误，完整 UI 提示与恢复入口留待后续应用工作流切片。
