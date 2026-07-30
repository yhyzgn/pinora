# 任务 003：建立 core/app 运行时骨架与单实例协议

- 状态：已完成
- 计划：`.context/plans/003_phase0_runtime_skeleton.md`
- 规模：中
- 依赖：`.context/plans/002_design_document.md`
- 生产行为变更：有（`main` 从 Hello World 变为运行时 bootstrap，仍无 GUI）

## 任务目标

将单 crate Hello World 演进为 workspace：`pinora-core` 承载命令/事件/错误/状态，`pinora-app` 承载 `AppRuntime`、单实例协议与入口；提供可离线通过的单元测试。

## 范围

- 根 `Cargo.toml` 改为 workspace；新增 `crates/pinora-core`、`crates/pinora-app`。
- 定义最小 `Command`、`DomainEvent`、错误码、`AppPhase`/`AppState`。
- 实现 `SingleInstance` trait（内存实现）、`AppRuntime` 启动/处理命令/关闭。
- 入口打印启动结果摘要；二次实例在协议层表现为转发 `Activate` 后退出。
- 同步 `AGENTS.md` 指针与 `.context/system/` 已验证事实。

## 非目标

- GPUI/Liora、系统托盘、全局热键、截图 Portal、文件锁单实例。
- 引入非 std 依赖。
- 完整领域模型（Pin/Annotation/OCR 结构体可留到后续任务）。

## 预期文件

- 修改：`Cargo.toml`、`AGENTS.md`、`.context/system/*`、本计划/任务。
- 新增：`crates/pinora-core/**`、`crates/pinora-app/**`。
- 删除或停用根 `src/main.rs`（迁入 app crate）。

## 验收标准

- `cargo test` 至少验证：首实例获取锁成功；次实例转发 Activate 且不进入 Running；Running 时处理 `Activate`/`Shutdown`；事件带 correlation。
- `cargo check` 与 `cargo run` 成功；`run` 输出表明应用已启动并优雅退出（无 GUI 循环也可立即退出或完成 demo bootstrap）。
- `context_bootstrap.py validate` 通过。
- core 不依赖 app。

## 验证

- `cargo test`
- `cargo check`
- `cargo run -p pinora-app`（或 workspace 默认 binary）
- `python .../context_bootstrap.py validate --root <仓库>`

## 风险与回滚

- 风险：workspace 迁移导致路径/IDE 混淆。缓解：保留包名清晰、更新 conventions。
- 风险：单实例内存实现被误当生产实现。缓解：类型命名 `InMemorySingleInstance`，overview 标注。
- 回滚：恢复单 crate `src/main.rs` 与旧 `Cargo.toml`；删除 `crates/`。

## 完成记录

- 状态：已完成（2026-07-30）。
- 实际变更：根目录为 workspace + 二进制 package；入口固定 `src/main.rs`；新增 `crates/pinora-core`（Command/Event/Error/State）与 `crates/pinora-app` 库（AppRuntime、内存单实例、fake 能力探测）；设计文档 §10 已同步。
- 实际验证：`cargo test --workspace` 14 passed；`cargo check --workspace` 通过；`cargo run` 输出 primary started → shutdown complete；`context_bootstrap.py validate` 通过。
- 未解决项：真实 OS 单实例锁与 IPC、GPUI/Liora 接入、托盘/热键。
