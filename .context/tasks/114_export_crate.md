# 任务 114：导出与剪贴板 crate 边界

- 状态：已完成
- 计划：`.context/plans/114_export_crate.md`
- 规模：大
- 依赖：任务 107、108、113 已完成。
- 生产行为变更：否；内部 crate 所有权迁移。

## 任务目标

让 `pinora-export` 独立拥有图像编码、原子文件保存、系统剪贴板适配和受监督导出任务，`pinora-app` 仅编排请求与消费结果。

## 范围

- 新增 `crates/pinora-export/{Cargo.toml,src/lib.rs,src/image_sink.rs,src/export_job.rs}`。
- 从 `pinora-app` 迁移 `image_sink.rs`、`export_job.rs` 及原有测试。
- 更新 workspace、app 依赖、导入和兼容 re-export。
- 更新设计文档及 `.context/system/{overview,conventions,risks}.md`。

## 预期文件

- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-export/Cargo.toml`
- `crates/pinora-export/src/{lib,image_sink,export_job}.rs`
- `crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/{lib,desktop_shell,platform,runtime,history_export}.rs`
- `AGENTS.md`、`.context/{plans,tasks}/114_export_crate.md`
- `docs/Pinora-开发设计文档.md`、`.context/system/{overview,conventions,risks}.md`

## 非目标

- `history_export.rs`、`history_load_job.rs`、`desktop_shell.rs` 的历史和窗口逻辑不迁移。
- 不改变公开类型名、状态格式、数据格式、剪贴板后端选择和错误语义。

## 验收标准

1. `pinora-export` 唯一拥有导出/剪贴板模块；app 删除旧模块并通过 re-export 兼容调用。
2. 编码格式、原子发布、取消/超时、worker 回收和剪贴板失败路径行为保持一致。
3. workspace、Clippy、Windows target、fmt、diff、ctx 校验通过。
4. 真实系统剪贴板、桌面权限和跨平台性能继续按风险记录。

## 验证

- `cargo test -p pinora-export -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：跨 crate 可见性调整遗漏、依赖回路、Linux 剪贴板子进程回收行为回归。
- 回滚：恢复 app 内两个模块和导入，移除 workspace 成员，不触碰用户文件格式。

## 完成记录

- 2026-08-03：新增 `crates/pinora-export`，将 `image_sink` 与 `export_job` 迁出 app；workspace 通过兼容 re-export 保持 `LocalImageSink`、`ExportJobService` 等调用面。
- 2026-08-03：`cargo test -p pinora-export -- --nocapture` 25 通过、1 个真实显示会话剪贴板测试忽略；`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --check`、`git diff --check` 和 `ctx validate` 通过。
- 2026-08-03：真实系统剪贴板权限、跨平台窗口/桌面性能与 GUI 端到端仍未验证，已登记 R-065；历史编排留在 app 以避免依赖回路。
