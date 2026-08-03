# 任务 115：历史工作流 crate 边界

- 状态：已完成
- 计划：`.context/plans/115_history_crate.md`
- 规模：大
- 依赖：任务 108、114 已完成。
- 生产行为变更：否；内部 crate 所有权迁移。

## 任务目标

让 `pinora-history` 独立拥有历史文件策略与异步加载服务，app 仅编排历史窗口、选择和结果应用。

## 范围

- 新增 `crates/pinora-history/{Cargo.toml,src/lib.rs,src/history_export.rs,src/history_load_job.rs}`。
- 从 `pinora-app` 迁移 `history_export.rs`、`history_load_job.rs` 及原有测试。
- 更新 workspace、app 依赖、导入和兼容 re-export。
- 更新设计文档及 `.context/system/{overview,conventions,risks}.md`。

## 预期文件

- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-history/Cargo.toml`
- `crates/pinora-history/src/{lib,history_export,history_load_job}.rs`
- `crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/{lib,desktop_shell}.rs`
- `AGENTS.md`、`.context/{plans,tasks}/115_history_crate.md`
- `docs/Pinora-开发设计文档.md`、`.context/system/{overview,conventions,risks}.md`

## 非目标

- 不迁移历史窗口、Panel 绘制、托盘菜单或通用任务监督底座。
- 不修改持久化数据形状、状态字符串、租户/权限语义或文件清理范围。

## 验收标准

1. `pinora-history` 唯一拥有历史策略和异步读取模块；app 删除旧实现并通过 re-export 兼容调用。
2. 历史插入、摘要验证、删除/清空、配额/保留期 tombstone 和 worker 结果门禁行为保持一致。
3. workspace、Clippy、Windows target、fmt、diff、ctx 校验通过。
4. 真实文件权限、断电、网络文件系统和 GUI/性能风险继续按上下文记录。

## 验证

- `cargo test -p pinora-history -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：公开 API 过宽、app 导入遗漏、HistoryStore 与导出输入依赖形成循环、关闭阶段 worker 未收敛。
- 回滚：恢复 app 内历史模块和导入，移除 workspace 成员；不触碰用户文件和索引格式。

## 完成记录

- 2026-08-03：新增 `crates/pinora-history`，将历史索引策略、受管 PNG 校验、删除/清空、配额/保留期 tombstone 清理和异步读取 worker 迁出 app；app 通过 crate 内兼容 re-export 保持 `desktop_shell` 调用面。
- 2026-08-03：`cargo test -p pinora-history -- --nocapture` 26 项通过；`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --check`、`git diff --check` 和 `ctx validate` 通过。
- 2026-08-03：真实文件权限、断电、网络文件系统、GUI/性能与跨平台窗口行为仍未验证，已登记 R-066。
