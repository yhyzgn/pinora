# 任务 045：历史窗口适配器拆分

- 状态：已完成
- 计划：`.context/plans/045_history_window_adapter.md`
- 规模：中
- 依赖：`.context/tasks/042_history_browser.md`、`.context/tasks/043_history_management.md`
- 生产行为变更：否；架构调整，保持既有历史交互与持久化行为。

## 范围

- 新增 `history_window` 内部 UI 适配器。
- 迁移历史窗口 winit/softbuffer 生命周期、预览缓存和 paint 逻辑。
- 收敛 `desktop_shell` 对历史窗口的直接资源访问。

## 任务目标

为后续 Overlay、设置和贴图窗口迁移建立无行为变化的模块拆分模式。

## 非目标

- 不重写历史面板逻辑、文件事务、Pin 创建、窗口风格或平台后端。

## 预期文件

- `crates/pinora-app/src/history_window.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `crates/pinora-app/src/lib.rs`
- `AGENTS.md`、`.context/plans/045_history_window_adapter.md`、`.context/tasks/045_history_window_adapter.md`
- `.context/system/overview.md`、`.context/system/risks.md`

## 验收标准

1. 历史窗口创建、关闭、resize、选中预览和 paint 由适配器承担。
2. Shell 继续持有文件/Pin/动作编排，拆分后历史搜索、单删和清空回归正常。
3. workspace fmt/check/Clippy/test、diff 检查和 ctx validate 通过。

## 验证

- `cargo test -p pinora-app history_browser::tests -- --nocapture`
- `cargo test -p pinora-app history_export::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：窗口/softbuffer borrow 错误可能只在真实事件循环暴露；保留生命周期 API 的错误传播和已有离线状态测试。
- 风险：拆分过程中复制状态导致预览陈旧；唯一缓存归适配器所有，shell 只在选择变化时刷新。
- 回滚：恢复原有状态结构和调用点，历史 codec/事务完全不受影响。

## 完成记录

- 2026-08-02：完成 `history_window` 内部适配器，迁移历史窗口资源、预览缓存、resize 和 paint；shell 保留动作/事务编排。
- 验证：`cargo test -p pinora-app history_browser::tests -- --nocapture`、`history_export::tests`、workspace fmt/check/严格 Clippy/test、diff 检查和 ctx validate 通过。
- 已知风险：拆分由离线测试和编译覆盖，未形成真实跨平台窗口生命周期证据。
