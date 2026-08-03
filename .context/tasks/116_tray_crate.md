# 任务 116：托盘适配 crate 边界

- 状态：已完成
- 计划：`.context/plans/116_tray_crate.md`
- 规模：大
- 依赖：任务 112、113、115 已完成。
- 生产行为变更：否；内部 crate 所有权迁移。

## 任务目标

让 `pinora-tray` 独立拥有 `tray-icon` 菜单/句柄和事件映射，app 只编排托盘动作。

## 范围

- 新增 `crates/pinora-tray/{Cargo.toml,src/lib.rs,src/tray.rs}`。
- 从 `pinora-app` 迁移 `tray.rs` 及原有测试。
- 更新 workspace、app 依赖、导入和兼容 re-export。
- 更新设计文档及 `.context/system/{overview,conventions,risks}.md`。

## 预期文件

- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-tray/Cargo.toml`
- `crates/pinora-tray/src/{lib,tray}.rs`
- `crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/{lib,desktop_shell}.rs`
- `AGENTS.md`、`.context/{plans,tasks}/116_tray_crate.md`
- `docs/Pinora-开发设计文档.md`、`.context/system/{overview,conventions,risks}.md`

## 非目标

- 不迁移托盘动作对应的截图、贴图、设置、历史、诊断业务工作流。
- 不改变 tray-only 窗口策略、菜单文本/状态、热键绑定或持久化数据。

## 验收标准

1. `pinora-tray` 唯一拥有 `tray-icon` 适配和现有测试；app 删除旧实现并通过 re-export 兼容调用。
2. 创建失败、菜单轮询、动态贴图列表、窗口候选清洗和固定反馈行为保持一致。
3. workspace、Clippy、Windows target、fmt、diff、ctx 校验通过。
4. 真实托盘、任务栏/Dock、菜单点击和跨平台窗口管理器风险继续按上下文记录。

## 验证

- `cargo test -p pinora-tray -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：GTK 条件依赖、菜单 ID 映射、TrayIconEvent 轮询和 app 关闭顺序的跨 crate 可见性遗漏。
- 回滚：恢复 app 内 `tray.rs` 和导入，移除 workspace 成员；不触碰截图、贴图或窗口生命周期。

## 完成记录

- 代码迁移：`pinora-tray` 已加入 workspace；`pinora-app` 删除旧 `tray.rs`，改用 crate re-export。
- 定向验证：`cargo test -p pinora-tray -- --nocapture`，15 项通过。
- 完整验证：`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --check`、`git diff --check`、`ctx validate` 均通过。
- 未覆盖风险：真实 Windows/macOS/X11/KDE Wayland tray、任务栏/Dock/分页器、重连、焦点和性能仍需授权原生桌面探针；回滚点为恢复 app 内 `tray.rs` 及直接依赖声明。
