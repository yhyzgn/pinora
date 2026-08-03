# 任务 099：脱敏诊断包导出

- 状态：已完成
- 计划：`.context/plans/099_sanitized_diagnostics_export.md`
- 规模：中
- 依赖：现有 `DiagnosticsPanel`、`TrayFeedback`、`AppTray` 和导出目录。
- 生产行为变更：是；新增一个用户主动触发的本地诊断文件和托盘菜单项，不联网。

## 任务目标

实现设计文档 4.10 要求的用户主动脱敏诊断包导出，并通过托盘动作完成闭环。

## 范围

- `crates/pinora-app/src/diagnostics_export.rs`
- `crates/pinora-app/src/tray.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- 相关上下文与风险文档

## 预期文件

- `AGENTS.md`
- `.context/plans/099_sanitized_diagnostics_export.md`
- `.context/tasks/099_sanitized_diagnostics_export.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. 报告字段仅来自固定白名单，敏感内容排除测试通过。
2. 托盘动作映射到一次导出，成功/失败反馈不携带路径或原始错误。
3. 原子写入成功/失败和临时文件清理测试通过。
4. workspace check、严格 Clippy、workspace 测试和上下文校验通过。

## 非目标

- 不做网络上传、自动收集、截图/剪贴板/OCR 内容导出、文件选择器或跨平台 GUI E2E。

## 风险与回滚

- 风险：真实托盘/目录权限/文件系统行为仍未验证；由 R-057 跟踪。
- 回滚：移除导出动作与模块，恢复现有诊断面板；不改动其它业务状态。

## 验证

- 诊断报告模型和敏感字段排除测试
- 原子写入成功/失败测试
- tray 动作映射测试
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`
- `git diff --check`

## 完成记录

- 2026-08-03 完成。新增 `crates/pinora-app/src/diagnostics_export.rs`，并接入 `tray.rs`、`tray_feedback.rs` 和 `desktop_shell.rs`。
- `cargo test -p pinora-app --lib -- --nocapture`：294 通过、2 个既有真实桌面测试忽略；`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace` 全部通过。
- `ctx validate` 与 `git diff --check` 通过；无新增依赖、无公共持久化格式变更、无网络副作用。
- 已知限制：CI/离线测试不证明真实 tray 点击、目录权限、原生文件系统原子性或辅助窗口任务栏/Dock/分页器行为。
