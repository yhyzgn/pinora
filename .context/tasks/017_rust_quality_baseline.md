# 任务 017：修复 Rustfmt 与 Clippy 质量门禁

- 状态：已完成
- 计划：`.context/plans/017_rust_quality_baseline.md`
- 规模：中
- 依赖：`.context/tasks/016_takeover_audit.md`
- 生产行为变更：无

## 任务目标

将当前 workspace 从“可编译但格式/静态检查失败”恢复到可重复执行的 fmt、Clippy 零告警和单元测试基线。

## 范围

- 对 workspace 应用 Rustfmt。
- 修复 `annotate.rs` 的 `too_many_arguments` 与安全除法告警。
- 修复 `capture.rs` 测试中的 `cloned_ref_to_slice_refs` 告警。
- 修复严格 Clippy 在首轮根因修复后继续暴露的局部表达式告警和 3 处矩形绘制函数边界。
- 执行完整静态和单元验证并更新完成记录。

## 非目标

- 不改变截图、贴图、OCR、导出或平台行为。
- 不处理 Windows target/GTK 架构问题；该问题留给平台边界重建任务。
- 不新增 suppressions 或依赖。

## 预期文件

- 多个现有 Rust 文件的 Rustfmt 机械格式化。
- `crates/pinora-core/src/annotate.rs`。
- `crates/pinora-core/src/capture.rs`。
- 当前计划、任务、系统概览和风险记录。

## 验收标准

- `cargo fmt --check` 通过。
- Clippy 零告警通过，首批 3 类告警及后续暴露的 52 个告警均被根因修复。
- `cargo check --workspace` 和 `cargo test --workspace` 继续通过。
- 不存在新增 `allow`/`expect` 类 warning suppression。

## 验证

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- Rustfmt 的 diff 较广但不改变语义；回滚时仅反转格式化提交或使用格式化工具恢复既有样式，不触及用户 `AGENTS.md` 追加内容。
- 标注绘制函数重构需保持像素输出；用既有标注测试和新增局部断言验证。

## 完成记录

- 状态：已完成（2026-08-01）。
- 已验证：`cargo fmt --check`、`git diff --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 全部通过；`cargo test --workspace` 中 40 个 app 测试和 35 个 core 测试通过，2 个真实桌面测试按条件忽略；上下文校验返回 `ok: true`。
- 实际变更：Rustfmt 格式化 workspace；`annotate.rs` 将线段端点值对象化并用 `NonZeroU64` 守卫马赛克分母；`capture.rs` 移除测试 clone；应用 Clippy 的机器建议，并将 `desktop_shell.rs` 和 `region_overlay.rs` 的矩形绘制边界收敛为 `PixelRect`/`PixelPoint`。
- 风险：本任务只建立 Rust 静态质量基线，不覆盖 GUI 端到端、真实多显示器、Windows/macOS 或后台子进程生命周期验证。
