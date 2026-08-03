# 任务 127：诊断报告功能 crate 边界

- 状态：已完成
- 计划：`.context/plans/127_diagnostics_crate.md`
- 规模：中
- 依赖：任务 112、113、118、126 已完成。
- 生产行为变更：否；诊断报告内部所有权迁移。

## 任务目标

让 `pinora-diagnostics` 唯一拥有诊断报告的固定字段白名单、脱敏设置摘要、渲染和原子文件
发布，让 app 继续拥有诊断面板、托盘动作、状态组装和固定反馈。

## 变更前记录

```text
目的：将 diagnostics_export.rs 从 pinora-app 迁入独立功能 crate，修复 app 对报告 IO 的越界所有权。
影响路径：托盘“导出诊断包”动作、诊断报告文件生成和失败反馈。
兼容性：报告头、字段顺序、字段值语义、文件命名和固定反馈保持不变；不改变接口、数据、状态、权限语义。
外部副作用：仍只写用户主动请求的本地受管导出目录；不联网、不读取用户内容、不启动外部进程。
回滚点：恢复 app 内 diagnostics_export.rs 与依赖，移除 pinora-diagnostics workspace 成员。
验证场景：报告金样、敏感字段排除、枚举设置、原子发布、临时文件清理、app 托盘导出回归。
```

## 范围

- 新增 `crates/pinora-diagnostics/{Cargo.toml,src/lib.rs}`。
- 迁移 `crates/pinora-app/src/diagnostics_export.rs` 的报告模型、渲染、原子写入和测试。
- 更新 workspace、app 依赖与 `desktop_shell` 导入/调用；删除 app 内重复实现。
- 更新设计文档、overview、conventions 和 risks。

## 非目标

- 不改变 `DiagnosticsPanel` 绘制、`DiagnosticsWindow`、`TrayAction`、能力探测、设置 schema、
  文件目录、报告格式、窗口策略、截图、OCR、导出、历史、贴图或托盘生命周期。
- 不新增依赖、原始 SQL、警告抑制、网络访问或真实 GUI 测试。

## 预期文件

- `AGENTS.md`
- `.context/plans/127_diagnostics_crate.md`
- `.context/tasks/127_diagnostics_crate.md`
- `Cargo.toml`
- `crates/pinora-diagnostics/{Cargo.toml,src/lib.rs}`
- `crates/pinora-app/Cargo.toml`
- `crates/pinora-app/src/{diagnostics_export,desktop_shell}.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-diagnostics` 独立编译且不反向依赖 app/desktop/tray/window。
2. 报告头、字段顺序、固定标签和敏感字段排除与迁移前一致。
3. 原子发布成功、失败清理和 app 导出反馈回归由测试覆盖。
4. workspace 测试、check、严格 Clippy、Windows target、fmt、diff 与 ctx validate 通过。

## 验证

- `cargo test -p pinora-diagnostics -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo run --quiet -- --version`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：报告格式或原子发布语义变化会使用户无法诊断或留下临时文件。
- 回滚：恢复 app 内模块和依赖，移除新 crate；不触碰报告格式、窗口、托盘和用户数据。

## 完成记录

- 2026-08-03 已完成。新增 `pinora-diagnostics` crate 并从 `pinora-app` 迁移诊断报告模型、
  固定平台/反馈标签校验、字段顺序、设置脱敏摘要、渲染、同目录临时文件与原子发布；删除
  app 内 `diagnostics_export` 模块。`desktop_shell` 继续从诊断面板和运行时能力组装固定
  `DiagnosticReportInput`，消费新 crate 并保留托盘反馈、窗口和 EventLoop 编排。
- 验证通过：`cargo test -p pinora-diagnostics -- --nocapture`（5 项）、
  `cargo test -p pinora-app --lib -- --nocapture`（22 项）、
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、
  `cargo run --quiet -- --version`（输出 `pinora 0.1.0`）、`cargo fmt --all -- --check`、
  `git diff --check` 与 `ctx validate`。
- 未覆盖风险：未连接真实共享基础设施；也未验证真实只读目录、断电/文件占用、托盘菜单、
  GUI、任务栏/Dock、HiDPI、窗口隔离或性能；由 R-078 及既有桌面风险继续跟踪。
