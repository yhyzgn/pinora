# 任务 130：导出会话状态模块

- 状态：已完成
- 计划：`.context/plans/130_export_session_state.md`
- 规模：中
- 依赖：任务 114、123、125、127、129 已完成。
- 生产行为变更：否；导出会话状态和纯判定的内部模块迁移。

## 任务目标

让 `pinora-app::export_session` 唯一拥有导出完成动作、待处理导出元数据、取消筛选、资产归属校验和
tray 操作映射，让 `desktop_shell` 继续独占运行时、文件名、任务、worker、Window/Surface、EventLoop 和
tray 副作用。

## 变更前记录

```text
目的：从 desktop_shell 抽出无副作用的导出会话值对象，降低唯一事件循环文件的职责密度。
影响路径：Overlay 复制/贴图/保存、贴图复制/保存、OCR 文本复制、文件导出取消和完成反馈。
兼容性：导出来源、文件格式和质量、任务 owner/AssetRef、历史候选、取消范围、tray 反馈和状态字符串不变。
外部副作用：无新增；导出 worker、文件/剪贴板、runtime、tray、历史、窗口和 EventLoop 行为保持原路径。
回滚点：恢复 desktop_shell 内类型/函数，移除 export_session 模块及上下文记录。
验证场景：Overlay 来源、三类 tray 操作、运行中文件取消、owner/资产匹配、冻结参数和全量回归。
```

## 范围

- 新增 `crates/pinora-app/src/export_session.rs`，迁移导出会话值对象、纯判定和状态测试。
- 更新 `crates/pinora-app/src/{lib,desktop_shell}.rs`，删除重复定义并保持现有副作用调用。
- 更新 `AGENTS.md`、计划/任务、设计文档、overview、conventions 和 risks。

## 非目标

- 不迁移编码、文件/剪贴板 IO、`ExportJobService`、文件名分配、历史写入、runtime、线程、Window/Surface、
  EventLoop、tray 或任务服务。
- 不新增依赖、原始 SQL、警告抑制、网络访问或真实 GUI/系统剪贴板测试。

## 预期文件

- `AGENTS.md`
- `.context/plans/130_export_session_state.md`
- `.context/tasks/130_export_session_state.md`
- `crates/pinora-app/src/{export_session,lib,desktop_shell}.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `export_session` 唯一拥有导出会话值对象和纯映射，且不创建窗口、不启动线程、不提交任务、不访问
   runtime、文件、剪贴板、tray 或历史。
2. `desktop_shell` 继续保留所有导出任务提交、结果处理、文件名、runtime、Window、EventLoop、tray 和
   恢复副作用，现有用户语义不变。
3. 状态模块边界测试和 app 回归通过；workspace、Clippy、Windows target、fmt、diff 与 ctx validate 通过。

## 验证

- `cargo test -p pinora-app export_session -- --nocapture`
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

- 风险：类型迁移遗漏导致贴图导出来源、取消筛选、owner/资产检查或 tray 映射变化。
- 回滚：恢复 desktop_shell 内定义并删除 export_session；不触碰编码、文件、剪贴板、历史、窗口、tray 或设置。

## 完成记录

- 实现：新增 `crates/pinora-app/src/export_session.rs`，迁移 `OverlayFinish`、
  `PendingExportAction`、`FrozenExportTarget`、`PendingExport`、导出来源、文件保存取消筛选、
  owner/资产匹配和 tray 操作映射；新模块不依赖 winit。
- 兼容性：贴图完成动作继续强制标注图；保存路径/格式/JPEG 质量仍在提交前冻结；取消继续只选择
  运行中的文件保存；任务 owner/AssetRef、历史候选、tray 状态和所有外部副作用保持原值与原时机。
- 验证：`cargo test -p pinora-app export_session -- --nocapture`（5 通过）；
  `cargo test -p pinora-app --lib -- --nocapture`（24 通过）；
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（通过，2 个真实桌面测试按既有条件忽略）；
  `cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo run --quiet -- --version`、
  `cargo fmt --all -- --check`、`git diff --check` 与 `ctx validate` 均通过。
- 风险与回滚：R-081 仍跟踪真实文件系统、系统剪贴板、tray、窗口管理器、焦点、HiDPI 和性能；
  回滚时恢复 `desktop_shell` 内定义并移除 `export_session`，不触碰编码、文件、剪贴板、历史、窗口、
  tray 或设置。
