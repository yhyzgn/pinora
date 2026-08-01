# 任务 035：接入设置运行时策略

- 状态：已完成
- 计划：`.context/plans/035_settings_runtime.md`
- 规模：小
- 依赖：`.context/tasks/033_versioned_settings_store.md`
- 生产行为变更：是；启动读取的贴图上限和默认不透明度开始影响运行时。

## 变更前记录

```text
目的：消除设置只读取不生效的假完成状态。
影响路径：AppRuntime、桌面壳新贴图初始化、src/main.rs 启动加载、上下文文档。
兼容性：保留现有命令、事件、截图像素、状态字符串、租户和权限语义；默认设置值与 033 一致。
外部副作用：仍只读取默认本地设置路径；不自动写回修复值，不访问真实共享基础设施。
回滚点：移除 runtime settings 字段和桌面壳初始化参数即可恢复 033 行为。
验证场景：自定义 pin_limit 拒绝超限贴图；自定义默认 opacity 应用于新贴图；手动透明度调整仍可用；损坏设置回退默认。
```

## 任务目标

在 runtime 保存校验后的 `AppSettings`，把 `pin_limit` 应用于 core 状态，把 `default_pin_opacity_percent` 应用于桌面壳新建贴图，并让入口不再丢弃加载结果。

## 范围

- `AppRuntime::with_settings`、设置访问器和单元测试。
- `run_desktop_shell`/`DesktopApp` 新贴图默认不透明度接入。
- `src/main.rs` 启动设置映射与上下文事实更新。

## 预期文件

- `crates/pinora-app/src/runtime.rs`、`desktop_shell.rs`。
- `src/main.rs`。
- `AGENTS.md`、`.context/plans/035_settings_runtime.md`、`.context/system/overview.md`、`.context/system/risks.md`。

## 非目标

- 设置 UI、热键热更新、主题渲染、跨平台路径和历史接入。

## 验收标准

- 自定义设置经过 `with_repaired_values` 后才可进入 runtime；`pin_limit` 与 core 限制一致。
- 新建贴图使用设置默认透明度，已有贴图和 `[`/`]` 调整不被覆盖。
- 缺失/损坏设置继续使用内存默认并保留原文件；启动日志不输出路径或敏感内容。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app runtime::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：主题仍未渲染，不能把 `ThemeMode` 当作已完成 UI；文档和日志明确区分。
- 风险：桌面壳构造器字段变化影响测试/调用方；通过 workspace check 和 targeted tests 覆盖。
- 回滚：移除新增设置接线，恢复默认 `max_pins=32` 与默认 opacity=1.0。

## 完成记录

- 状态：已完成（2026-08-02）。
- 实际变更：入口保留 `SettingsLoad` 结果并注入 `AppRuntime`；缺失/无效仍只使用内存默认，不自动写回。
- 实际变更：`pin_limit` 驱动 core 贴图上限；`default_pin_opacity_percent` 驱动桌面壳新建贴图初值，手动透明度快捷键保持原有行为。
- 验证：`cargo test -p pinora-app runtime::tests -- --nocapture` 11/11、desktop shell 设置转换测试通过；workspace check、严格 Clippy、workspace tests（app 89、core 50）、fmt/diff/ctx 门禁通过。
- 未覆盖项：主题尚未渲染，设置 UI/保存交互、热键热更新和跨平台配置目录仍需后续独立任务。
