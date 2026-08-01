# 任务 034：建立历史索引与 tombstone 存储

- 状态：已完成
- 计划：`.context/plans/034_history_index.md`
- 规模：中
- 依赖：`.context/tasks/033_versioned_settings_store.md`
- 生产行为变更：是；新增显式历史索引 API，但不改变默认截图、贴图或导出动作。

## 变更前记录

```text
目的：把历史条目的生命周期、去重、配额和删除顺序固化为可测试边界。
影响路径：pinora-core 历史领域模型、pinora-app 历史索引文件存储、上下文文档。
兼容性：不改变现有命令、状态字符串、截图像素、租户或权限语义；新增文件格式 schema=1。
外部副作用：仅在调用方明确提供的本地目录读写索引文件；不删除用户外部文件，不连接真实基础设施。
回滚点：移除新增 history 模块即可回到 033 设置存储状态。
验证场景：新增/重复、tombstone、compact、配额裁剪、损坏、未知 schema、原子覆盖。
```

## 任务目标

新增可校验的历史条目/索引领域模型与原子本地 codec，固化去重、配额淘汰、tombstone 和 compact 的顺序语义。

## 范围

- `HistoryEntry`、`HistoryIndex`、内容摘要、OCR 状态和 tombstone 操作。
- 固定 magic/schema、最大条数/字段长度/文件长度的 codec。
- `HistoryStore::load/save` 和不覆盖损坏源文件的原子保存。
- 纯离线测试与上下文事实更新。

## 预期文件

- `crates/pinora-core/src/history.rs`、`lib.rs`。
- `crates/pinora-app/src/history_store.rs`、`lib.rs`。
- `AGENTS.md`、`.context/plans/034_history_index.md`、`.context/system/overview.md`、`.context/system/risks.md`。

## 非目标

- GUI 历史页、缩略图解码、自动删除 PNG、跨平台配置目录和真实桌面探针。

## 验收标准

- 相同摘要与尺寸只保留最新条目；不同图像可并存。
- `mark_deleted` 只写 tombstone，`compact` 才物理移除；条目文件名必须是相对路径且禁止 `..`。
- 非法输入、未知 schema、超长文件均返回 invalid，原文件字节保持不变。

## 风险与回滚

- 风险：历史 API 尚未接入桌面导出和 UI，不能被误报为完整历史功能；通过非目标和系统风险登记隔离。
- 风险：文件删除失败时索引不能直接 compact；通过 tombstone 返回值和调用方责任约束。
- 回滚：删除新增模块和导出符号，恢复 033 设置存储版本；不修改既有导出文件。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-core history::tests -- --nocapture`
- `cargo test -p pinora-app history_store::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 完成记录

- 状态：已完成（2026-08-02）。
- 实际变更：新增 `HistoryEntrySpec`、`HistoryEntry`、`HistoryIndex` 和 `HistoryInsert`；相同内容摘要与字节长度只保留最新活动条目，配额淘汰和显式删除先标记 tombstone，`compact` 才移除记录。
- 实际变更：新增 `HistoryStore` 版本化二进制 codec（`PINHIST`、schema=1、CRC32、总长度/字段上限）与原子保存/读取验证；文件名只允许受管目录单个相对组件。
- 验证：定向历史测试 8/8；`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo fmt --check`、`git diff --check` 与 ctx validate 通过。
- 未覆盖项：桌面事件循环尚未调用历史 API，未执行真实文件删除或历史 UI/复用场景；相关能力保留在后续独立任务。
