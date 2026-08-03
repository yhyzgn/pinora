# 任务 097：历史最大磁盘占用

- 状态：已完成
- 计划：`.context/plans/097_history_max_bytes.md`
- 规模：中
- 依赖：096 历史保留天数、041 设置持久化、042/043 历史 tombstone 清理。
- 生产行为变更：是；受管历史 PNG 将按持久化的最大字节配额在启动、设置保存和新增历史后安全淘汰。

## 任务目标

为历史记录增加生产级可配置磁盘字节配额，完成设置 v8 迁移、设置面板交互和既有历史策略协调接入。

## 范围

- `AppSettings` 增加 16 MiB 至 64 GiB 的 `history_max_bytes`，默认 1 GiB。
- `SettingsStore` 从 v7 升级到 v8，v1-v7 迁移并对非法 v8 容量逐字段修复。
- 设置面板增加容量行，使用 64 MiB 步长和 MiB/GB 标签，保持固定布局和保存/取消语义。
- DesktopApp 启动与设置保存把容量传给 `HistoryStore`，继续复用数量/保留期/tombstone 协调器。
- 增加 codec、面板、历史配额和失败回滚回归测试，更新 system/风险/完成记录。

## 非目标

- 不修改历史索引 schema，不改变历史导出格式、OCR、预览、再贴图、再编辑、用户外部导出或窗口策略。
- 不读取磁盘剩余空间，不新增后台清理线程或平台 API。

## 预期文件

- `crates/pinora-core/src/settings.rs`
- `crates/pinora-app/src/settings_store.rs`
- `crates/pinora-app/src/settings_panel.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `crates/pinora-app/src/history_export.rs`
- `AGENTS.md`
- `.context/plans/097_history_max_bytes.md`
- `.context/tasks/097_history_max_bytes.md`
- `.context/system/{overview.md,risks.md,conventions.md}`

## 验收标准

- v1-v7 读取默认 1 GiB；v8 精确字节往返；非法容量仅修复容量字段。
- 容量超额按最旧优先产生 tombstone，保存失败回滚，清理失败保留重试状态。
- 面板边界、步进、标签、布局、保存失败与取消通过。

## 验证

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 首次迁移可能按默认 1 GiB 清理超额受管历史；只允许已持久 tombstone 的受管直属 PNG 被删除。
- 回滚仅移除 v8 字段和运行时容量传递，恢复 `u64::MAX`，不重写历史索引。

## 完成记录

- 完成时间：2026-08-03。
- 实现结果：设置 schema v8 追加精确字节容量，默认 1 GiB、范围 16 MiB..=64 GiB；v1-v7 迁移默认容量，非法 v8 容量逐字段修复。
- 实现结果：设置面板支持容量行、MiB/GB 标签、64 MiB 键盘/鼠标步进和固定布局；启动与设置保存将容量传入历史存储。
- 实现结果：既有历史策略协调器按最旧优先处理容量超额 tombstone，继续执行原子索引、受管 PNG 白名单清理和失败重试保护。
- 回归覆盖：v8 往返、v7 迁移、非法容量修复、面板边界、容量下调淘汰、索引失败回滚、外部/嵌套/活动同名保护均通过。
- 门禁：`cargo fmt --all -- --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 290 通过、2 忽略；core 90 通过）、`cargo check --workspace --target x86_64-pc-windows-msvc`、`git diff --check` 与 `ctx validate` 全部通过。
- 风险：真实磁盘中断恢复、只读/网络文件系统、跨平台桌面窗口和 tray 行为未由离线门禁证明，详见 `.context/system/risks.md` 的 `R-055`。
