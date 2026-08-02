# 任务 046：设置窗口适配器拆分

- 状态：已完成
- 计划：`.context/plans/046_settings_window_adapter.md`
- 规模：中
- 依赖：`.context/tasks/041_settings_panel.md`、`.context/tasks/045_history_window_adapter.md`
- 生产行为变更：否；架构调整，保持既有设置交互和持久化行为。

## 范围

- 新增 `settings_window` 内部 UI 适配器。
- 迁移设置窗口资源、草稿面板、存储调用、resize 和 paint。
- 让 desktop shell 保留保存后策略、历史配额和窗口动作编排。

## 任务目标

消除 `desktop_shell` 对设置窗口资源和保存草稿的直接持有，为后续 Overlay/贴图拆分建立一致边界。

## 非目标

- 不修改设置字段、schema、主题渲染或真实平台控件。

## 预期文件

- `crates/pinora-app/src/settings_window.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `crates/pinora-app/src/lib.rs`
- `AGENTS.md`、`.context/plans/046_settings_window_adapter.md`、`.context/tasks/046_settings_window_adapter.md`
- `.context/system/overview.md`、`.context/system/risks.md`

## 验收标准

1. 设置窗口创建、关闭、输入、草稿、保存调用、resize 和 paint 由适配器承担。
2. shell 只在适配器保存成功后应用 runtime 与历史清理策略。
3. settings panel/runtime/history 回归和 workspace 质量门禁通过。

## 验证

- `cargo test -p pinora-app settings_panel::tests -- --nocapture`
- `cargo test -p pinora-app runtime::tests -- --nocapture`
- `cargo test -p pinora-app history_export::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：草稿和保存结果分离后错误处理回归；沿用既有状态机测试和保存失败分支。
- 风险：softbuffer 生命周期只由编译/离线测试覆盖；保留真实桌面风险。
- 回滚：回迁窗口状态和存储调用，不改变 settings.bin 或 runtime 策略。

## 完成记录

- 2026-08-02：完成 `settings_window` 内部适配器，迁移窗口资源、面板草稿、存储调用、resize 和 paint；shell 保持保存后策略应用。
- 验证：`settings_panel::tests` 4/4、`runtime::tests` 11/11、`history_export::tests` 13/13；workspace fmt/check/严格 Clippy/test、diff 检查和 ctx validate 通过。
- 已知风险：适配器拆分未取代真实平台窗口、主题与无障碍探针。
