# 任务 126：历史保留期时间策略

- 状态：已完成
- 计划：`.context/plans/126_history_retention_time_policy.md`
- 规模：小
- 依赖：任务 108、115、125 已完成。
- 生产行为变更：否；内部历史时间策略所有权迁移。

## 任务目标

让 `pinora-history` 唯一拥有历史保留期截止时间计算，让 app 继续仅编排设置、策略执行和
用户可见反馈。

## 变更前记录

```text
目的：将历史保留期的当前时间和截止时间计算从 desktop_shell 迁入 pinora-history。
影响路径：启动、设置保存和导出成功后触发的历史保留期策略调用。
兼容性：不改变接口、数据、状态、租户或权限语义；0 天、饱和回推和时间不可用语义保持不变。
外部副作用：无；纯时间策略不访问文件、窗口、线程、网络或共享基础设施。
回滚点：恢复 desktop_shell 内时间/截止函数，移除 pinora-history 对应导出。
验证场景：0 天、正常回推、早期时间、最大 `u16` 输入、系统时间不可用和 app 历史回归。
```

## 范围

- 新增 `crates/pinora-history/src/retention.rs`。
- 迁移历史保留期的当前时间、截止时间和边界逻辑及测试。
- 切换 app 的历史策略调用，删除本地副本。
- 更新 crate 导出、设计/系统/风险文档。

## 非目标

- 不改变历史策略、索引/文件持久化、设置 schema、窗口、EventLoop、截图、OCR、导出、贴图或托盘。
- 不引入任何新依赖或外部基础设施访问。

## 预期文件

- `AGENTS.md`
- `.context/plans/126_history_retention_time_policy.md`
- `.context/tasks/126_history_retention_time_policy.md`
- `crates/pinora-history/src/{lib,retention}.rs`
- `crates/pinora-app/src/{lib,desktop_shell}.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. history crate 唯一拥有保留期时间策略；app 删除本地时间计算副本。
2. 零天、正常回推、早期时钟、最大 `u16` 输入与时钟不可用均由 history 测试覆盖。
3. app 仍独占策略调用时机、窗口刷新、错误反馈、Window/Surface 和唯一 EventLoop。

## 验证

- `cargo test -p pinora-history -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：截止时间偏差会改变受管历史的过期时间。
- 回滚：恢复 app 内时间函数，移除 history crate 对应导出；不触碰索引、文件、设置、窗口、数据格式或其他功能。

## 完成记录

- 2026-08-03 已完成。新增 `pinora-history/src/retention.rs`，将保留期当前时间读取、
  天数到毫秒转换和饱和截止时间计算迁出 `desktop_shell`；零天不读取时钟，当前时间不可用
  时不产生截止点。`desktop_shell` 保留导出命名所需的 `SystemTime`，并通过 crate-private
  re-export 消费新策略函数；原本 app 内的重复时间测试已删除。
- 验证通过：`cargo test -p pinora-history -- --nocapture`（32 通过）、
  `cargo test -p pinora-app --lib -- --nocapture`（25 通过）、
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、
  `cargo run --quiet -- --version`（输出 `pinora 0.1.0`）、`cargo fmt --check`、
  `git diff --check` 与 `ctx validate`。
- 未覆盖风险：没有连接真实共享基础设施；也未验证真实时钟跳变、断电/权限/网络文件系统、
  历史窗口刷新、GUI、tray-only、任务栏/Dock、HiDPI 或性能，继续由 R-054 与 R-077 跟踪。
