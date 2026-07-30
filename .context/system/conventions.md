# 代码规范与上下文规则

## 当前工程约定

- Cargo workspace：唯一二进制入口为根 `src/main.rs`（package `pinora`）；领域在 `pinora-core`，编排库在 `pinora-app`；后续 crate 按设计文档 `crates/pinora-*` 拆分。
- 依赖方向：`src/main.rs` → `pinora-app` → 下层；`pinora-core` 不得依赖 app、UI 或平台适配器。
- 命令表示意图、事件表示已发生事实；事件须带 `event_id`、`correlation_id`、`occurred_at_ms`；日志不得写入截图像素、OCR 全文或凭据。
- 平台能力通过 trait 注入：`CaptureProvider`、`SingleInstance`、`CapabilityProbe`；测试用 `FakeCaptureProvider` / `InMemorySingleInstance`；生产入口用 `OsSingleInstance` + fake 捕获直至真实后端就绪。
- 修改公共模块、类型或函数前，使用 `rg` 搜索全部引用，并运行 `cargo check --workspace` 与相关测试。

## 验证命令

- 工程元数据：`cargo metadata --no-deps --format-version 1`
- 编译：`cargo check --workspace`
- 测试：`cargo test --workspace`
- 运行探针：`cargo run`
- 上下文完整性：`python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## Git 身份（固定，勿再询问）

本仓库本地固定使用以下提交身份，代理在提交/改写历史时直接使用，不得改用其他账号，也不得再次向用户确认：

```text
user.name  = Neo
user.email = yhyzgn@gmail.com
```

配置位置：仓库 `.git/config`（`git config --local`）。若缺失，提交前自动写回上述值。

## 文档与变更规则

- 稳定事实写入 `system/`；阶段顺序写入 `plans/`；一个有边界、可验证、可回滚的动作写入 `tasks/`。
- 证据与不确定性分开记录；设计文档中的建议不能直接升级为实现事实。
- 所有面向人员的上下文文档使用中文；路径、命令和无法翻译的技术标识符除外。
- 目前没有持久化数据访问层；后续数据库或 ORM 变更必须遵守仓库级 SQL 红线并补充隔离测试。
