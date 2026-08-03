# 任务 113：自绘桌面面板 crate 边界

- 状态：已完成
- 计划：`.context/plans/113_desktop_panels.md`
- 规模：大
- 依赖：任务 109/111/112 的 desktop 交互、窗口和呈现边界。
- 生产行为变更：否；内部 crate 所有权迁移。

## 变更前记录

```text
目的：将自绘面板和无窗口视觉控制从 app 单体迁入 desktop，隔离 UI 数据/布局/像素逻辑。
影响路径：desktop crate、app lib re-export、设置/历史/诊断窗口与 desktop shell 调用、设计和上下文文档。
兼容性：面板尺寸、布局、键盘/鼠标行为、主题、固定反馈、历史和贴图菜单语义不改变。
外部副作用：无新增窗口、线程、进程、文件、网络或系统注册。
回滚点：恢复 app 内五个模块与导入，移除 desktop 导出。
验证场景：主题切换、设置导航、历史筛选/确认、诊断脱敏、选区读数布局、贴图菜单命中与像素绘制。
```

## 任务目标

让 `pinora-desktop` 拥有纯自绘 UI，app 只保留窗口宿主和业务服务。

## 范围

- 迁移 `settings_panel`、`history_browser`、`diagnostics_panel`、`overlay_selection_readout`、`pin_context_menu` 至 `crates/pinora-desktop/src/`。
- 更新 desktop crate 导出和 app 根 crate 内兼容模块 re-export；不改窗口适配器接口。
- 更新设计文档和 `.context/system/{overview,conventions,risks}.md`。

## 非目标

- 不迁移窗口资源、历史加载/删除、设置保存、诊断文件导出、托盘句柄、Overlay 会话或贴图生命周期。

## 预期文件

- `crates/pinora-desktop/src/{lib,settings_panel,history_browser,diagnostics_panel,overlay_selection_readout,pin_context_menu}.rs`
- `crates/pinora-app/src/lib.rs`
- `AGENTS.md`、`.context/{plans,tasks}/113_desktop_panels.md`
- `docs/Pinora-开发设计文档.md`、`.context/system/{overview,conventions,risks}.md`

## 验收标准

1. desktop crate 唯一拥有五个纯 UI 模块和原有测试，app 删除旧实现。
2. 现有面板/菜单/读数行为和 app 窗口适配调用兼容。
3. 严格 workspace、Clippy、Windows target、fmt、diff 和 ctx 门禁通过。
4. 真实 GUI、HiDPI、输入法、焦点、tray/taskbar 和性能继续按风险记录。

## 验证

- `cargo test -p pinora-desktop -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：公开绘制/状态接口遗漏、app 模块 re-export 遮蔽或跨 crate 可见性过宽；真实平台输入/绘制仍未验证。
- 回滚：恢复 app 内五个模块和导入；不触碰用户数据、窗口策略、任务或持久化格式。

## 完成记录

- 2026-08-03：已将 `settings_panel`、`history_browser`、`diagnostics_panel`、`overlay_selection_readout` 和 `pin_context_menu` 迁入 `pinora-desktop`，`pinora-app` 通过 `pinora_desktop` 兼容 re-export 继续调用这些纯 UI 模块。
- 2026-08-03：已核对 `crates/pinora-desktop` 目录树与依赖，确认 crate 仅依赖 `pinora-core` 与 `winit`；`cargo tree -p pinora-desktop --depth 1`、`cargo tree -p pinora-app --depth 1`、`cargo test -p pinora-desktop -- --nocapture`（77 通过）、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`git diff --check` 与 `ctx validate` 已执行并通过。
- 2026-08-03：设计文档、系统全景、规范、风险与本任务/计划状态已同步为完成；真实 GUI、HiDPI、输入法、焦点、tray/taskbar 和性能仍需原生桌面探针。
