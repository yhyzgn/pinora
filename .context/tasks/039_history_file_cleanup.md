# 任务 039：清理配额淘汰的受管历史文件

- 状态：已完成
- 计划：`.context/plans/039_history_file_cleanup.md`
- 规模：中
- 依赖：`.context/tasks/034_history_index.md`、`.context/tasks/037_history_export_integration.md`
- 生产行为变更：是；历史配额淘汰产生的 tombstone 将在安全条件满足时删除对应受管 PNG 并压缩索引。

## 变更前记录

```text
目的：让历史索引的 tombstone 与 Pinora 管理导出文件形成可重试的清理闭环。
影响路径：history 领域压缩 API、桌面导出完成后的历史清理、离线测试与上下文文档。
兼容性：不改变截图、PNG 内容、复制语义、公共命令、owner/generation 门禁或用户主动导出行为。
外部副作用：仅删除 runtime.export_dir() 直属且已 tombstone 的应用受管文件；不访问共享服务。
回滚点：移除清理调用和受限压缩 API，保留历史写入与 tombstone 记录，文件不再自动删除。
验证场景：删除成功、文件已缺失、活动同名引用保护、删除失败、索引保存失败回滚。
```

## 任务目标

在历史写入已原子保存 tombstone 后，对符合白名单的文件执行幂等删除；仅在确认删除完成后 compact 对应 tombstone，并将压缩后的索引原子保存。

## 范围

- `HistoryIndex` 的受限 tombstone 压缩能力。
- `pinora-app` 的受管历史文件清理器与离线测试。
- `desktop_shell` 在受监督 SavePng 历史写入成功后的清理接线与脱敏日志。
- 当前工作指针、上下文事实与风险记录。

## 预期文件

- `crates/pinora-core/src/history.rs`。
- `crates/pinora-app/src/history_export.rs`、`crates/pinora-app/src/desktop_shell.rs`。
- `AGENTS.md`、`.context/plans/039_history_file_cleanup.md`、`.context/tasks/039_history_file_cleanup.md`、`.context/system/overview.md`、`.context/system/risks.md`。

## 非目标

- 历史 UI、缩略图、再次贴图/编辑、用户手动删除、保留天数和后台定时清理。
- 任意外部目录扫描、目录递归删除、用户主动导出文件管理、真实 GUI 或断电恢复探针。

## 验收标准

- 仅删除 tombstone 对应的直属受管文件；活动条目引用相同文件名时不删除也不 compact。
- 文件不存在可安全 compact；删除失败或类型冲突不 compact，tombstone 保留。
- 删除后索引保存失败会恢复内存 tombstone；诊断不包含绝对路径、像素或 OCR 全文。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-core history::tests -- --nocapture`
- `cargo test -p pinora-app history_export::tests -- --nocapture`
- `cargo test -p pinora-app history_store::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：同名活动条目可能引用新文件。缓解：清理前收集活动文件名，冲突时保留 tombstone。
- 风险：文件删除成功而索引保存失败。缓解：恢复内存 tombstone，磁盘索引仍保持已持久的删除意图，后续可确认缺失文件并重试 compact。
- 回滚：移除桌面清理调用与清理器；历史索引保留 tombstone，停止自动删除文件。

## 完成记录

- 状态：已完成（2026-08-02）。
- 实际变更：`pinora-core::HistoryIndex::compact_confirmed_tombstones` 只移除调用方确认外部文件删除完成的 tombstone；未确认条目保持可恢复状态。
- 实际变更：`pinora-app::history_export::cleanup_history_tombstones` 使用受管目录直属 PNG 白名单，支持文件、符号链接和缺失文件的幂等处理；活动条目同名保护、不可删除目录保留 tombstone；桌面 SavePng 历史记录成功后接入清理并输出脱敏计数。
- 实际变更：清理后 `HistoryStore::save` 失败恢复内存索引，已删除文件不会被伪装成索引提交成功，磁盘 tombstone 可在后续启动/导出后重试。
- 验证：`cargo fmt --check`；核心 history 定向测试 5/5；应用 history_export 定向测试 8/8；history_store 4/4；Overlay 回归 9/9；`cargo check --workspace`；严格 Clippy；`cargo test --workspace`（151 通过，2 忽略）；`git diff --check`；均通过。
- 未覆盖风险：真实文件权限、平台符号链接策略、断电恢复和 GUI 历史操作没有验证；不等同于完整历史 UI 或用户导出目录清理支持。
