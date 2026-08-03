# 任务 112：桌面呈现状态 crate 边界

- 状态：已完成
- 计划：`.context/plans/112_desktop_presentation.md`
- 规模：中
- 依赖：任务 111 窗口策略边界。
- 生产行为变更：否；内部 crate 所有权迁移。

## 变更前记录

```text
目的：把共享面板主题、tray 能力摘要和受控反馈从 app 单体剥离。
影响路径：pinora-desktop 模块、app 的面板/托盘/诊断导入、设计文档和上下文事实。
兼容性：主题模式、系统外观回退、固定反馈文本、错误码和脱敏边界均不改变。
外部副作用：无新增窗口、线程、进程、网络、文件或系统注册。
回滚点：恢复 app 内三个模块和导入，移除 desktop 对应模块。
验证场景：Light/Dark/System 主题、未知系统外观、能力摘要覆盖、反馈错误码和敏感字段隔离。
```

## 任务目标

建立 `pinora-desktop` 的呈现状态边界，让业务模块只消费稳定 token 和受控文案。

## 范围

- 迁移 `crates/pinora-app/src/{panel_theme,tray_capabilities,tray_feedback}.rs` 至 `crates/pinora-desktop/src/`。
- 更新 desktop crate manifest/lib、app manifest/lib 及面板/托盘/诊断/desktop shell 导入。
- 更新设计文档、`.context/system/{overview,conventions,risks}.md`。

## 非目标

- 不迁移 tray-icon/GTK 菜单句柄、窗口 EventLoop、设置/历史/诊断面板业务状态。

## 预期文件

- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-desktop/src/{lib,panel_theme,tray_capabilities,tray_feedback}.rs`
- `crates/pinora-app/src/{lib,settings_panel,history_browser,diagnostics_panel,settings_window,history_window,diagnostics_window,tray,desktop_shell}.rs`
- `AGENTS.md`、`.context/{plans,tasks}/112_desktop_presentation.md`
- `docs/Pinora-开发设计文档.md`、`.context/system/{overview,conventions,risks}.md`

## 验收标准

1. desktop crate 唯一拥有三个纯呈现模块，app 不保留旧实现且公共导出兼容。
2. 面板、托盘、诊断定向测试和 workspace/Clippy/Windows target/fmt/diff/ctx 门禁通过。
3. 不将静态主题/反馈测试描述为真实平台托盘、窗口或系统主题验收。

## 验证

- `cargo test -p pinora-desktop -- --nocapture`
- `cargo test -p pinora-app --lib panel_theme -- --nocapture`
- `cargo test -p pinora-app --lib tray_capabilities -- --nocapture`
- `cargo test -p pinora-app --lib tray_feedback -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：跨 crate 可见性扩大、固定反馈导入遗漏或系统主题事件边界变化。
- 回滚：恢复 app 内三个模块和导入，移除 desktop 导出；不改变用户设置、历史、截图或托盘数据。

## 完成记录

- `pinora-desktop` 已成为面板主题、系统外观解析、tray 能力摘要及固定反馈/错误码映射的唯一实现；app 不再声明旧三个模块。
- app 通过 `pub(crate)` 模块 re-export 兼容现有 `crate::panel_theme`、`crate::tray_capabilities`、`crate::tray_feedback` 调用，未改变外部公开接口或用户可见文案。
- 验证通过：desktop 定向 43 项、workspace 全量离线测试、workspace check、严格 Clippy、Windows target check、fmt、diff check 和 `ctx validate`。
- 已知缺口：真实 tray/窗口管理器/系统主题事件/HiDPI/性能尚未在本地验证；无生产行为变更。
