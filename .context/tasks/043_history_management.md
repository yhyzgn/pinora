# 任务 043：历史搜索与全量清理

- 状态：已完成
- 计划：`.context/plans/043_history_management.md`
- 规模：大
- 依赖：`.context/tasks/042_history_browser.md`
- 生产行为变更：是；新增历史筛选和清空入口。

## 范围

- 历史面板搜索输入、过滤列表和清空确认状态。
- 全量活动条目 tombstone 标记、原子索引保存、受管 PNG 清理和失败恢复。
- 桌面历史窗口快捷键/按钮接线与最小纯逻辑测试。

## 任务目标

在 042 的历史安全边界上交付搜索和全量清理，不牺牲条目身份、原子索引或可恢复删除语义。

## 非目标

- 不实现标签、OCR 全文检索、再次编辑、原生控件或真实桌面自动化。

## 预期文件

- `crates/pinora-app/src/history_browser.rs`
- `crates/pinora-app/src/history_export.rs`
- `crates/pinora-app/src/history_store.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `AGENTS.md`、`.context/plans/043_history_management.md`、`.context/tasks/043_history_management.md`
- `.context/system/overview.md`、`.context/system/risks.md`

## 验收标准

1. 历史窗口可输入搜索文本，结果按既有时间顺序过滤，清空搜索恢复完整列表。
2. 搜索状态不改变持久化索引，筛选后 Enter/Delete 仍作用于原始条目标识。
3. 清空需确认；确认后先持久化 tombstone，再执行受管文件清理；任一步失败保留可重试状态。
4. 空历史、大小写/空白、取消确认、索引保存失败、删除失败和同名保护均有离线测试。
5. workspace fmt/check/Clippy/test、diff 检查和 `ctx validate` 通过。

## 验证

- `cargo test -p pinora-app history_browser::tests -- --nocapture`
- `cargo test -p pinora-app history_export::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：清空动作涉及多个文件，部分删除失败会留下 tombstone；复用既有可重试清理器并在状态栏显示未完成数量。
- 风险：搜索输入会扩大自绘窗口事件分支；保持状态机纯逻辑并锁定命中/过滤测试。
- 回滚：移除搜索和清空入口，不改变 042 的单条事务和历史 codec。

## 完成记录

- 2026-08-02：完成历史搜索、筛选后复用/删除、清空确认和全量 tombstone 清理接线。
- 2026-08-02：补充清空成功、索引保存失败回滚以及历史面板搜索/确认状态测试。
- 验证：`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check`、`ctx validate` 均通过。
- 已知风险：搜索和清空仍依赖自绘 softbuffer 窗口，未验证真实平台焦点、HiDPI、读屏和权限故障注入。
