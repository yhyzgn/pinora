# 任务 117：OCR 任务服务 crate 边界

- 状态：已完成
- 计划：`.context/plans/117_ocr_service_crate.md`
- 规模：中
- 依赖：任务 110 已完成。
- 生产行为变更：否；内部 crate 所有权迁移。

## 任务目标

把 app 内独立的 OCR worker、缓存和结果门禁服务迁入 `pinora-ocr`，收窄 `pinora-app` 的职责。

## 范围

- 将 `crates/pinora-app/src/ocr_job.rs` 迁移为 `crates/pinora-ocr/src/job.rs`。
- 更新 `pinora-ocr/src/lib.rs`、`pinora-app/src/lib.rs` 和模块声明。
- 更新设计文档、系统事实、约束和风险记录。
- 迁移原有测试，不改变业务行为。

## 非目标

- 不重构 `desktop_shell` 的 OCR UI 编排。
- 不改变 Tesseract 适配、设置 schema、剪贴板或托盘菜单。

## 预期文件

- `AGENTS.md`
- `.context/plans/117_ocr_service_crate.md`
- `.context/tasks/117_ocr_service_crate.md`
- `crates/pinora-ocr/src/{lib,job}.rs`
- `crates/pinora-app/src/lib.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验证

- `cargo test -p pinora-ocr -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 验收标准

1. `pinora-ocr` 唯一拥有 OCR 任务服务、缓存、runner 和既有单元测试；app 删除 `ocr_job` 模块。
2. app 仍只通过单一 EventLoop 触发 OCR、提供当前 owner/asset 并消费有效结果。
3. 所列定向与完整验证通过，真实本地引擎和桌面性能风险持续记录。

## 风险与回滚

- 风险：跨模块类型导出或测试模块路径不一致导致编译失败；worker 退出回收语义被误改。
- 回滚：恢复 app 内文件和导出，移除 `pinora-ocr::job`；保留其余 crate 拆分。

## 完成记录

- 代码迁移：`crates/pinora-app/src/ocr_job.rs` 已迁至 `crates/pinora-ocr/src/job.rs`；app 删除本地模块，改由 `pinora-ocr` re-export 兼容其库 API。
- 定向验证：`cargo test -p pinora-ocr -- --nocapture`，26 项通过；`cargo test -p pinora-app --lib -- --nocapture`，57 项通过。
- 完整验证：`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --check`、`git diff --check`、`ctx validate` 均通过。
- 未覆盖风险：真实 Tesseract 模型、外部子进程压力、窗口关闭竞态、词框 GUI、任务栏/Dock/分页器和性能仍需授权原生桌面探针；回滚点为恢复 app 内 `ocr_job.rs`。
