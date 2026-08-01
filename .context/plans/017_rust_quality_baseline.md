# 计划 017：恢复 Rust 质量门禁基线

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/017_rust_quality_baseline.md`

## 目标

在不改变用户可观察功能和平台方案的前提下，恢复可重复的 Rust 格式与零告警 Clippy 基线，使后续架构迁移拥有可信的静态质量门禁。

## 非目标

- 不把格式化或 Clippy 通过描述为桌面应用已达到生产级。
- 不重写 `desktop_shell`、替换截图后端或新增跨平台能力。
- 不新增 lint suppression、依赖或 feature 开关。

## 约束

- 使用 `cargo fmt` 做机械格式化；语义修复必须小而可审查。
- 修复 Clippy 根因，不能使用 `#[allow]`、`#[expect]` 或 lint 级别降级。
- 保持既有单元测试行为；只补与修复直接相关的测试。

## 依赖关系

- 依赖 `.context/tasks/016_takeover_audit.md` 已完成的质量门禁证据。

## 阶段

1. 应用 Rustfmt 并确认无超出格式的变化。
2. 修复首批 Clippy 根因，并处理严格检查继续暴露的等价表达式和绘制函数参数边界。
3. 反复执行 fmt、check、Clippy、workspace 测试和上下文校验。

## 检查点

- 每个 Clippy 修复有明确的语义等价依据。
- `git diff --check` 通过，用户已有 `AGENTS.md` 追加内容不被覆盖。
- 不启动图形桌面、不触发真实截图或共享外部服务。

## 计划级风险

- Rustfmt 会产生广泛机械 diff；通过单独记录并避免混入架构重写控制审查噪音。
- 清除首批 Clippy 报告后继续暴露了旧桌面壳中的表达式告警；已用机器建议和局部值对象重构消除，未借由 suppression 隐藏。

## 完成标准

- `cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 全部通过。
- 未添加 warning suppression。
- `.context` 状态与实际输出一致。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际修复：`annotate.rs` 使用 `PixelPoint` 表达线段端点、用 `NonZeroU64` 表达马赛克样本数非零不变量；`capture.rs` 测试移除不必要 clone。严格 Clippy 随后暴露的 49 个生产代码和 3 个测试告警，已通过可机器验证的表达式修复及 3 处矩形绘制值对象化消除。
- 验证：`cargo fmt --check`、`git diff --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 和上下文校验全部通过。
- 未解决范围：跨平台 GTK 依赖、Linux/KDE 平台绑定、GUI 端到端覆盖和外部子进程生命周期仍按审计风险登记处理，未被本任务声明为已解决。
