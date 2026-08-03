# 任务 096：历史保留天数

- 状态：已完成
- 计划：`.context/plans/096_history_retention_days.md`
- 规模：中
- 依赖：041 设置持久化、042/043 历史 tombstone 清理、049 历史复用、092 设置 v6 codec。
- 生产行为变更：是；历史受管 PNG 可按保存的保留天数在启动、设置保存和新增历史后安全过期。

## 任务目标

将设计规格 4.9 的保留天数约束接入现有历史事务：用户在设置面板配置天数，运行时基于真实 Unix 时间计算截止点，先持久化过期 tombstone，再进行严格白名单文件清理。

## 范围

- `AppSettings` 增加 1..=3650 的 `history_retention_days` 与默认 30 天，设置 schema 升级到 v7，保留 v1-v6 解码迁移。
- 设置面板增加可键盘和鼠标编辑的 `HISTORY RETENTION` 行，并保持固定布局无重叠。
- `HistoryIndex` 增加纯截止时间 tombstone 标记；历史导出模块集中协调配额、过期和清理事务。
- 启动、设置成功保存、受管 PNG 写入成功后执行协调；系统时间不可读取时只跳过时间淘汰，既有配额和 tombstone 清理继续工作。
- 更新系统全景、风险、计划和任务完成记录。

## 预期文件

- `crates/pinora-core/src/{settings.rs,history.rs}`
- `crates/pinora-app/src/{settings_store.rs,settings_panel.rs,history_export.rs,desktop_shell.rs}`
- `AGENTS.md`
- `.context/plans/096_history_retention_days.md`
- `.context/tasks/096_history_retention_days.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不新增最大磁盘占用设置或改变现有 `HISTORY_MAX_BYTES = u64::MAX` 的容量策略。
- 不迁移历史索引 schema，不改变历史预览/再贴图/再编辑/删除/清空行为，不删除用户外部导出。
- 不新增窗口、线程、后台任务、平台 API、依赖、网络或权限路径。

## 验收标准

1. v7 记录往 v6 末尾追加保留天数；v1-v6 都迁移至默认值，v7 非法数值只修复该字段，保存仍原子写入并回读验证。
2. 过期判定严格为“活动条目的创建时间早于截止时间”；数量和容量策略与既有行为兼容，tombstone 和排序不变量保持。
3. 新 tombstone 保存失败时内存完整回滚；清理只作用于直属受管 PNG，失败可重试，不会删外部、嵌套或活动同名文件。
4. 启动、成功设置保存和受管 PNG 新增后均协调策略；时间读取失败不执行时间删除，也不阻断其他历史流程。
5. 设置面板布局可容纳新行，鼠标/键盘导航、保存失败、取消与既有主题行为不回归。

## 验证

- `cargo test -p pinora-core settings -- --nocapture`
- `cargo test -p pinora-core history -- --nocapture`
- `cargo test -p pinora-app --lib settings_store::tests -- --nocapture`
- `cargo test -p pinora-app --lib settings_panel::tests -- --nocapture`
- `cargo test -p pinora-app --lib history_export::tests -- --nocapture`
- `cargo test -p pinora-app --lib desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：本机墙钟可能跳变或不可读取。缓解：领域层只比较应用层给出的截止时间；应用层无法取得可信当前时间时跳过时间淘汰，不把未知时间当作过期。
- 风险：索引和文件在进程中断间暂时分叉。缓解：先原子写 tombstone，文件操作/最终压缩失败均保留可重试 tombstone；现有受管路径和活动同名保护不变。
- 风险：设置保存成功而历史策略索引写失败。缓解：持久化设置仍真实反映用户选择，历史内存索引回滚并记录受控延后状态，后续启动/新增/保存会重试协调。
- 回滚：移除新设置字段、v7 codec 和协调调用即可恢复当前历史行为；不需要迁移或重写既有历史索引及受管 PNG。

## 完成记录

- 完成时间：2026-08-03。
- 实现结果：`AppSettings`/`SettingsStore` 升级 v7，新增 1..=3650 天历史保留期与默认 30 天；设置面板增加 `HISTORY RETENTION` 行，边界和布局有纯逻辑测试。
- 实现结果：历史索引按 Unix 毫秒截止点将过期活动条目标记为 tombstone；桌面壳在启动、设置原子保存成功和受管 PNG 成功写入后复用统一协调器，先保存索引再清理直属受管 PNG。系统时间异常时跳过时间淘汰，索引/删除失败保持可重试状态。
- 回归覆盖：v1-v7 codec、非法字段修复、严格时间边界、策略保存失败回滚、外部/嵌套/活动同名文件保护、设置面板交互与桌面截止计算均通过。
- 门禁：`cargo fmt --all -- --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 286 通过、2 忽略；core 90 通过）、`cargo check --workspace --target x86_64-pc-windows-msvc`、`git diff --check` 与 `ctx validate` 全部通过。
- 风险：离线门禁不证明系统时钟跳变、断电/只读/网络文件系统、历史窗口竞态、真实桌面性能或任务栏/Dock/分页器行为；详见 `.context/system/risks.md` 的 `R-054`。
