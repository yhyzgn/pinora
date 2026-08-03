# 任务 131：历史加载会话状态模块

- 状态：已完成
- 计划：`.context/plans/131_history_session_state.md`
- 规模：小
- 依赖：任务 115、126、128、129、130 已完成。
- 生产行为变更：否；历史加载会话状态和纯判定的内部模块迁移。

## 任务目标

让 `pinora-app::history_session` 唯一拥有历史加载意图、请求、活动请求、准备类型映射和结果资产匹配，
让 `desktop_shell` 继续独占真实历史读取任务、窗口、贴图/编辑器、错误反馈和 EventLoop。

## 变更前记录

```text
目的：从 desktop_shell 抽出无副作用的历史加载会话值对象，继续降低唯一事件循环文件的职责密度。
影响路径：历史预览、Enter 重新贴图、编辑器重开、异步结果接收与取消后的结果丢弃。
兼容性：历史条目、任务 owner/AssetRef、generation、超时、取消、准备类型、面板状态、贴图和编辑器语义不变。
外部副作用：无新增；历史读取 worker、文件、窗口、EventLoop、tray、历史索引和 runtime 行为保持原路径。
回滚点：恢复 desktop_shell 内类型/函数，移除 history_session 模块及上下文记录。
验证场景：三种意图映射、匹配结果、错误 job/owner、条目 generation 变化和全量回归。
```

## 范围

- 新增 `crates/pinora-app/src/history_session.rs`，迁移历史加载会话值对象、纯映射和回归测试。
- 更新 `crates/pinora-app/src/{lib,desktop_shell}.rs`，删除重复定义并保持现有副作用调用。
- 更新 `AGENTS.md`、计划/任务、设计文档、overview、conventions 和 risks。

## 非目标

- 不迁移历史读取 worker、文件读取、索引/清理、窗口、Surface、EventLoop、tray、贴图/编辑器创建或错误反馈。
- 不新增依赖、原始 SQL、警告抑制、网络访问或真实 GUI/文件系统测试。

## 预期文件

- `AGENTS.md`
- `.context/plans/131_history_session_state.md`
- `.context/tasks/131_history_session_state.md`
- `crates/pinora-app/src/{history_session,lib,desktop_shell}.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `history_session` 唯一拥有历史加载会话值对象和纯映射，且不创建窗口、不启动线程、不读取文件、不提交/轮询任务、不访问 runtime、tray 或历史索引。
2. `desktop_shell` 保留历史任务启动/轮询/结果消费、窗口/贴图/编辑器、错误反馈和所有外部副作用，现有用户语义不变。
3. 会话模块边界测试和 app 回归通过；workspace、Clippy、Windows target、fmt、diff 与 ctx validate 通过。

## 验证

- `cargo test -p pinora-app history_session -- --nocapture`
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

- 风险：迁移遗漏导致过期历史结果被接受，或改变取消、预览、贴图/编辑器与面板错误状态。
- 回滚：恢复 desktop_shell 内定义并删除 history_session；不触碰历史文件、索引、worker、窗口、贴图、tray 或设置。

## 完成记录

- 实现：新增 `crates/pinora-app/src/history_session.rs`，迁移 `HistoryLoadIntent`、
  `HistoryLoadRequest`、`ActiveHistoryLoad`、意图到 `HistoryLoadPreparation` 映射和结果资产匹配。
  新模块不依赖 winit，不读取文件、不启动/轮询 worker、不创建窗口或调用 tray/runtime。
- 兼容性：请求启动前仍确认当前选中条目；完成结果继续只在 job id、owner、image id 与 generation
  同时匹配时交付。历史读取、取消、超时、窗口/贴图/编辑器、错误反馈和 EventLoop 的副作用时机未改变。
- 验证：`cargo test -p pinora-app history_session -- --nocapture`（3 通过）；
  `cargo test -p pinora-app --lib -- --nocapture`（26 通过）；
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、fmt、diff 与 `ctx validate` 均通过。
- 风险与回滚：R-082 继续跟踪真实历史文件、worker 时序、窗口管理器、焦点、HiDPI、tray-only 和性能；
  回滚时恢复 `desktop_shell` 内值对象并移除 `history_session`，不触碰持久化文件、索引、worker、
  窗口、贴图、tray 或设置。
