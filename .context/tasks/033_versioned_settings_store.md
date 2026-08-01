# 任务 033：建立版本化设置模型与原子存储

- 状态：已完成
- 计划：`.context/plans/033_versioned_settings_store.md`
- 规模：中
- 依赖：`.context/tasks/028_atomic_png_export.md`
- 生产行为变更：是；启动可读取本地设置，缺失使用默认值，损坏/未知版本保留文件并报告受限状态。

## 目的

将设计文档的“版本化设置、逐字段回退、原子替换”从目标文字变为独立可测试边界，消除当前所有运行配置只存在内存或临时目录的状态。

## 变更前记录

```text
目的：创建版本化设置领域值、无依赖 codec 和原子文件存储。
影响路径：pinora-core 设置模型、pinora-app 设置存储/入口、当前上下文文档。
兼容性：当前没有设置持久化格式；不改公共命令、截图、标注、任务、租户或权限语义。
外部副作用：仅在明确传入的本地测试目录读写文件；不访问真实桌面、网络、共享服务或用户实际配置目录。
回滚点：删除 settings 模块及入口加载；保留现有运行时默认行为。
验证场景：默认值、字段回退、缺失文件、损坏、未知 schema、原子覆盖、临时文件清理。
```

## 任务目标

新增 settings 领域模型与稳定 schema；应用层实现有长度上限的二进制 codec、原子保存及 load outcome。入口只接入缺失/可用/受损状态，不在本任务中新增设置 UI 或热键变更。

## 范围

- 设置模型、默认值和字段校验。
- 原子本地文件 codec/store 及离线测试。
- 最小启动加载边界与上下文记录。

## 非目标

- 不实现设置窗口、完整热键配置、主题渲染、历史、用户目录迁移或跨平台目录声明。
- 不新增依赖、数据库、原始 SQL、网络或外部基础设施访问。

## 预期文件

- `crates/pinora-core/src/settings.rs`、`lib.rs`。
- `crates/pinora-app/src/settings_store.rs`、`lib.rs`，必要时 `src/main.rs`。
- 当前计划、任务、系统概览、风险登记和 `AGENTS.md`。

## 验收标准

- 默认/损坏/未知版本/字段非法都有稳定、可测试结果，损坏文件不被覆盖。
- 原子保存使用同目录临时文件、同步、rename 和读取校验。
- 不新增依赖或警告抑制；完整离线质量门禁通过。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-core settings::tests -- --nocapture`
- `cargo test -p pinora-app settings_store::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：手写 codec 漏检长度或版本。缓解：固定魔数、最大长度、逐段检查和损坏输入测试。
- 风险：写入失败破坏旧设置。缓解：同目录临时文件、同步、rename 和读取验证，失败时保留旧文件。
- 风险：启动时静默覆盖损坏配置。缓解：区分 missing 与 invalid，invalid 只回退内存默认值并保留文件。
- 回滚：移除设置模块和入口加载，无需迁移已有数据。

## 完成记录

- 状态：已完成（2026-08-02）。
- 初始证据：入口以单实例运行目录派生导出目录，仓库没有 settings 模型、配置文件、版本号、损坏恢复或设置 UI；设计文档的设置/存储仍是目标能力。
- 实际变更：新增 `AppSettings`、`ThemeMode` 和 `SettingsRepairs`；数字字段不合法时逐字段回退，theme/schema 由 codec 严格校验而非静默转换。
- 实际变更：新增 `SettingsStore`，格式包含固定 magic、schema、theme、历史上限、贴图上限和不透明度；严格检查记录长度、magic、schema 和 theme。保存经同目录临时文件、同步、rename 和读取验证完成，临时文件由 RAII 清理。
- 实际变更：`src/main.rs` 只读取 XDG/HOME 回退路径并输出不含路径的状态。缺失和无效设置均只使用内存默认，不会在启动时创建或覆盖用户文件。
- 验证：`cargo test -p pinora-core settings::tests -- --nocapture` 通过 2/2；`cargo test -p pinora-app settings_store::tests -- --nocapture` 通过 4/4；`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`git diff --check` 与上下文校验均通过。workspace 为 app 82 项通过、2 项真实桌面测试忽略，core 46 项通过。
- 未覆盖项：设置尚未提供 UI、主题渲染、热键热更新或真实用户配置目录探针；035 已接入贴图数量上限和新贴图默认不透明度，历史、迁移、热键和跨平台目录策略仍属后续任务。
