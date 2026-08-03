# 任务 137：历史加载会话 crate

- 状态：已完成
- 计划：`.context/plans/137_history_session_crate.md`
- 规模：小
- 依赖：任务 115、126、131、136 已完成。
- 生产行为变更：否；历史加载纯会话契约的 crate 边界迁移。

## 任务目标

让 `pinora-history` 唯一承载 `HistoryLoadIntent`、`HistoryLoadRequest`、`ActiveHistoryLoad`、
意图到 `HistoryLoadPreparation` 的映射和当前历史选择的结果资产门禁；让 app 只消费该 crate，
不改变桌面壳的真实副作用。

## 变更前记录

```text
目的：将稳定的历史加载会话值对象从 app 私有模块提升到既有历史功能 crate，降低 desktop_shell 与 app 的职责密度。
影响路径：历史预览、重新贴图、编辑器的请求快照、worker 准备类型映射和 job/owner/generation 结果门禁。
兼容性：HistoryEntry、image id、generation、JobId、JobOwner、准备类型、文件格式、读取策略和 UI 行为不变。
外部副作用：无新增；文件读取、worker、Window/Surface、Panel、tray、贴图、编辑器、OCR、导出、runtime 和 EventLoop 保持原路径。
回滚点：移除 pinora-history::history_session 并恢复 pinora-app::history_session。
验证场景：意图映射、相同/不同 generation、相同/不同 job/owner、依赖图和全量回归。
```

## 范围

- 在 `crates/pinora-history/src/` 新增历史加载会话模块，迁移三项纯逻辑回归测试。
- 更新 `pinora-history` 公开导出、`pinora-app` 模块声明和 `desktop_shell` 导入。
- 更新 `AGENTS.md`、计划/任务、设计文档和 `.context/system/`。

## 非目标

- 不迁移历史读取服务、worker、窗口、Panel、贴图、编辑器、EventLoop、tray 或任务服务。
- 不新增依赖、原始 SQL、警告抑制、网络访问或真实 GUI 测试。

## 预期文件

- `AGENTS.md`
- `crates/pinora-history/src/{lib,history_session}.rs`
- `crates/pinora-app/src/{lib,desktop_shell}.rs`
- `.context/{plans,tasks}/137_history_session_crate.md`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-history` 唯一拥有迁移的会话类型、映射/门禁函数与三项回归测试，新增模块不依赖 app、desktop 或 winit。
2. `pinora-app` 不再有 `history_session` 内部模块；shell 的读取、worker、窗口、tray、贴图、编辑器和结果副作用不变。
3. 定向测试、workspace、Clippy、Windows target、fmt、diff 与 ctx validate 通过。

## 验证

- `cargo test -p pinora-history -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo run --quiet -- --version`
- `cargo fmt --all -- --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：可见性迁移可能破坏过期 job、错误 owner 或 generation 的拒绝语义。
- 回滚：删除 `pinora-history::history_session` 并恢复 app 私有模块；不触碰历史索引、受管 PNG、worker、
  窗口、贴图、tray、OCR、导出或设置。

## 完成记录

- 已完成：在 `pinora-history` 新增 `history_session` 模块，迁移 `HistoryLoadIntent`、
  `HistoryLoadRequest`、`ActiveHistoryLoad`、意图到 `HistoryLoadPreparation` 的映射和结果资产门禁及
  3 项回归测试。app 私有 `history_session` 已删除，`desktop_shell` 改为直接导入 crate 契约；文件读取、
  worker、窗口、贴图、编辑器、tray、错误反馈和 EventLoop 未迁移。
- 兼容性：`HistoryEntry`、image id、generation、`JobId`、`JobOwner`、准备类型、读取策略和 UI 行为保持原值；
  结果继续同时要求相同 job、`JobOwner::History(image_id)` 与当前条目的 image id/generation。
- 已验证：`cargo test -p pinora-history -- --nocapture`（35 通过）、`cargo test -p pinora-app --lib -- --nocapture`
  （12 通过）、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、
  `cargo run --quiet -- --version`、`cargo fmt --all -- --check`、`git diff --check`、
  `cargo metadata --no-deps --format-version 1` 与 `ctx validate` 均通过。
- 风险与回滚：R-082 继续跟踪真实文件、worker、窗口管理器、tray-only、任务栏/Dock、焦点、HiDPI 与性能；
  回滚时移除 `pinora-history::history_session` 并恢复 app 私有模块，不触碰历史索引、受管 PNG、worker、
  窗口、贴图、tray、OCR、导出或设置。
