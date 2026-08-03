# 任务 108：本地存储 crate

- 状态：已完成
- 计划：`.context/plans/108_storage_crate.md`
- 规模：中
- 依赖：任务 104 设置 schema v9、任务 105 平台边界、任务 107 通用任务监督。
- 生产行为变更：否；内部 crate 所有权迁移。

## 任务目标

建立 `pinora-storage`，迁移 `settings_store`、`history_store`、`export_name`，让设置/历史文件 codec 与命名策略独立于 app UI 和任务实现。

## 变更前记录

```text
目的：把版本化设置、历史索引和受管导出命名的本地文件边界从桌面壳中剥离。
影响路径：Cargo workspace、app 的设置/历史/导出命名导入、根入口、上下文和设计文档。
兼容性：接口 / 数据 / 状态 / 租户 / 权限均不改变；保持 schema v9、历史索引格式、原子写入与文件名格式。
外部副作用：无新增外部服务、网络或系统注册；仍只访问既有用户管理目录。
回滚点：恢复 app 内三个模块和直接引用，移除 pinora-storage。
验证场景：设置 v1-v9 迁移与往返、历史损坏/校验/配额、原子保存失败、命名冲突与扩展名冻结。
```

## 范围

- 新增 `crates/pinora-storage/{Cargo.toml,src/{lib,settings_store,history_store,export_name}.rs}`。
- 更新 workspace、app manifest、app lib re-export、settings_window/history_export/desktop_shell/root main 导入。
- 更新设计文档与 `.context/system/{overview,conventions,risks}.md`。

## 非目标

- 不迁移历史清理事务、图像编码、剪贴板、OCR、任务监督或窗口。
- 不改变数据格式、路径、配额和错误语义。

## 预期文件

- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-storage/Cargo.toml`、`crates/pinora-storage/src/*.rs`
- `crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/{lib,settings_window,history_export,desktop_shell}.rs`
- `src/main.rs`
- `AGENTS.md`、`.context/{plans,tasks}/108_storage_crate.md`
- `docs/Pinora-开发设计文档.md`、`.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-storage` 仅依赖 `pinora-core`，拥有三个模块的唯一实现与测试。
2. app 不再声明或编译旧 `settings_store.rs`、`history_store.rs`、`export_name.rs`，兼容 re-export 仍可用。
3. 设置、历史和命名定向测试以及 workspace 严格门禁、Windows target、ctx 校验通过。
4. 真实权限、断电、网络文件系统和 GUI 行为缺口明确记录。

## 验证

- `cargo test -p pinora-storage -- --nocapture`
- `cargo test -p pinora-app --lib settings_store -- --nocapture`
- `cargo test -p pinora-app --lib history_store -- --nocapture`
- `cargo test -p pinora-app --lib export_name -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：历史清理编排对 `HistoryStore` 的跨 crate 可见性、命名器 public API 和根入口导入遗漏。
- 回滚：恢复 app 内实现和导入，移除新 crate；不触碰用户设置、历史文件或导出目标。

## 完成记录

- `pinora-storage` 已成为设置 schema、历史索引 codec、原子文件存储和受管文件名分配的唯一实现；crate 仅依赖 `pinora-core` 与标准库。
- app 已切换到 `pinora_storage`，旧 `settings_store.rs`、`history_store.rs`、`export_name.rs` 已删除，公共 re-export 保持兼容。
- 验证通过：`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（1 + 226/1 ignored + 25/1 ignored + 90 + 7 + 21 + 28）、`cargo check --workspace`、严格 Clippy、Windows target check、fmt、diff check 和 `ctx validate`。
- 已知缺口：真实权限、断电持久性、只读/网络文件系统、GUI/托盘/任务栏与性能未在本地离线环境验证；无生产行为变更。
