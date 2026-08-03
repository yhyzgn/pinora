# 任务 118：应用运行时工作流 crate 边界

- 状态：已完成
- 计划：`.context/plans/118_runtime_crate.md`
- 规模：中
- 依赖：任务 105、106、107、114、117 已完成。
- 生产行为变更：否；内部 crate 所有权迁移。

## 任务目标

把 app 内独立的命令分发、状态变更、单实例 bootstrap/forward/shutdown 和事件发布迁入 `pinora-runtime`。

## 范围

- 新增 `crates/pinora-runtime/{Cargo.toml,src/lib.rs,src/runtime.rs}`。
- 迁移 `crates/pinora-app/src/runtime.rs` 及既有运行时测试。
- 将 `CapabilityProbe` trait 迁入 runtime；app 保留 `RuntimeCapabilityProbe` 与 `FakeCapabilityProbe` 实现。
- 更新 workspace、app、根入口、desktop shell 的导入和兼容 re-export。
- 更新设计文档、系统事实、约束和风险。

## 非目标

- 不重构 `desktop_shell.rs` 的 Overlay/贴图绘制与窗口生命周期。
- 不改变 core 领域命令、事件、状态或数据格式。
- 不引入第二个单实例锁、EventLoop 或后台服务。

## 预期文件

- `AGENTS.md`
- `.context/plans/118_runtime_crate.md`
- `.context/tasks/118_runtime_crate.md`
- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-runtime/Cargo.toml`
- `crates/pinora-runtime/src/{lib,runtime}.rs`
- `crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/{lib,platform,desktop_shell}.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-runtime` 唯一拥有 `AppRuntime` 与命令/事件工作流，app 删除旧 runtime 模块。
2. `CapabilityProbe` 只有一份定义，真实探测仍由 app 实现；根入口和 desktop shell 行为保持不变。
3. 原有 runtime 测试迁移后全部通过，workspace 与目标平台静态门禁通过。

## 验证

- `cargo test -p pinora-runtime -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：泛型 trait 可见性、测试 fake 依赖和 app re-export 遗漏导致编译或公共 API 回归。
- 回滚：恢复 `crates/pinora-app/src/runtime.rs` 与模块声明，移除 `pinora-runtime` workspace 成员和依赖；不改动领域模型。

## 完成记录

- 代码迁移：新增 `crates/pinora-runtime`，`crates/pinora-app/src/runtime.rs` 已迁入 `src/runtime.rs`；app 删除旧模块并改为公开 re-export。
- 定向验证：`cargo test -p pinora-runtime -- --nocapture`，14 项通过；`cargo test -p pinora-app --lib -- --nocapture`，43 项通过。
- 完整验证：`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --check`、`git diff --check`、`ctx validate` 均通过。
- 未覆盖风险：真实 Windows/macOS/Linux 单实例、权限、IPC、tray-only、任务栏/Dock/分页器、焦点和性能仍需授权原生桌面探针；回滚点为恢复 app 内 runtime 模块。
