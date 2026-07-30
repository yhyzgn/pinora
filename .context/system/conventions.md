# 代码规范与上下文规则

## 当前工程约定

- 这是 Rust 2024 单 crate 二进制；入口保持在 `src/main.rs`，新增能力应按职责拆分模块，避免继续堆积入口逻辑。
- 当前没有既有分层、错误类型或异步约定；引入框架或依赖前，先在计划中记录版本、平台边界和替代方案。
- 修改公共模块、类型或函数前，使用 `rg` 搜索全部引用，并运行 `cargo check` 与相关测试。

## 验证命令

- 工程元数据：`cargo metadata --no-deps --format-version 1`
- 编译：`cargo check`
- 测试：`cargo test`
- 运行探针：`cargo run`（当前应输出 `Hello, world!`）
- 上下文完整性：`python /home/neo/.claude/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 文档与变更规则

- 稳定事实写入 `system/`；阶段顺序写入 `plans/`；一个有边界、可验证、可回滚的动作写入 `tasks/`。
- 证据与不确定性分开记录；设计文档中的建议不能直接升级为实现事实。
- 所有面向人员的上下文文档使用中文；路径、命令和无法翻译的技术标识符除外。
- 目前没有持久化数据访问层；后续数据库或 ORM 变更必须遵守仓库级 SQL 红线并补充隔离测试。
