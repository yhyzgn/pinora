# 任务 037：接入导出成功后的历史记录

- 状态：已完成
- 计划：`.context/plans/037_history_export_integration.md`
- 规模：中
- 依赖：`.context/tasks/034_history_index.md`、`.context/tasks/027_desktop_export_job_integration.md`
- 生产行为变更：是；成功 PNG 导出会产生本地历史索引条目。

## 变更前记录

```text
目的：让历史索引只记录真实完成且通过任务门禁的 PNG 导出。
影响路径：desktop_shell 历史加载/导出完成处理、上下文文档与离线测试。
兼容性：不改变 PNG 内容、复制语义、任务 owner/generation 门禁、命令和状态字符串。
外部副作用：启动只读历史索引；导出成功后写入应用管理目录的 history.bin；不删除文件、不访问共享服务。
回滚点：移除历史字段/候选和完成回调，PNG 导出恢复 036 前行为。
验证场景：成功保存入历史、复制任务不入历史、失败/陈旧任务不入历史、索引保存失败回滚。
```

## 任务目标

为 SavePng 导出保留元数据候选，在成功 completion 后创建 `HistoryEntry` 并原子保存；历史保存失败恢复原索引。

## 范围

- `DesktopApp` 历史 store/index 生命周期。
- `PendingExport` 的轻量历史候选和 SavePng completion 接线。
- 纯逻辑测试、上下文记录和工作指针更新。

## 预期文件

- `crates/pinora-app/src/desktop_shell.rs`。
- `AGENTS.md`、`.context/plans/037_history_export_integration.md`、`.context/tasks/037_history_export_integration.md`、`.context/system/overview.md`、`.context/system/risks.md`。

## 非目标

- 历史 UI、文件删除/恢复事务、再次贴图/编辑和真实桌面验证。

## 验收标准

- SavePng 成功后历史条目使用相对文件名、图像摘要、来源显示器、源矩形和 `AssetRef` generation。
- CopyImage/CopyText、失败、超时、owner 关闭和陈旧结果不会写历史。
- history.bin 损坏时保留源文件并以空内存索引启动；索引写失败不改变内存索引。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app history_store::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：导出路径必须属于管理目录；候选构造拒绝非单组件文件名，失败只记录脱敏摘要。
- 风险：历史保存失败不能让 PNG 导出失败；通过保存前 clone、失败恢复 index 隔离。
- 回滚：删除 `HistoryStore` 接线和候选字段，保留 034 孤立索引 API 供后续使用。

## 完成记录

- 状态：已完成（2026-08-02）。
- 实际变更：新增 `history_export` 纯逻辑边界，在 SavePng 输入、受管导出目录、单组件 `.png` 文件名、图像 ID 与 `AssetRef` 一致时才冻结历史候选；外部路径、嵌套路径、非 PNG 和复制任务均被拒绝。
- 实际变更：`DesktopApp` 管理 `HistoryStore`/`HistoryIndex` 生命周期。受监督 SavePng 完成后再次读取已发布 PNG，创建包含相对文件名、SHA-256 摘要、显示器、选区与 generation 的条目并原子保存。保存失败会恢复该次内存索引，且不改变已成功的 PNG 导出结果。
- 验证：`cargo fmt --check`；`cargo test -p pinora-app history_export::tests -- --nocapture`（4/4）；`cargo test -p pinora-app history_store::tests -- --nocapture`（4/4）；`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`（9/9）；`cargo check --workspace`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo test --workspace`（146 通过，2 忽略）；`git diff --check`；ctx validate，均通过。
- 未覆盖风险：没有真实桌面窗口或文件系统权限探针；损坏索引只保证启动时不自动覆盖，后续成功导出是否替换索引属于正常持久化行为；历史 UI、文件删除事务、失效文件扫描和贴图复用仍不在范围内。
