# 任务 041：可操作设置面板

- 状态：已完成
- 计划：`.context/plans/041_settings_panel.md`
- 规模：大
- 依赖：`.context/tasks/033_versioned_settings_store.md`、`.context/tasks/035_settings_runtime.md`
- 生产行为变更：是；新增显式用户设置入口和运行时策略热应用。

## 范围

- 新增纯逻辑设置面板状态机、字段布局、步进和命中测试。
- 在 `desktop_shell` 中创建独立设置窗口，支持鼠标和键盘。
- 保存通过既有 `SettingsStore::save` 原子发布；成功后更新 `AppRuntime` 与桌面策略。
- 关闭、Esc 或取消不保存草稿；错误不泄露路径或用户内容。

## 任务目标

把既有设置 codec 和 runtime 接入提升为用户可操作、可回滚的设置窗口，并锁定保存成功/失败、取消和配额清理的事务边界。

## 非目标

- 不改 `settings.bin` schema 或默认值语义。
- 不声称已完成系统主题跟随、原生控件无障碍和真实跨平台 GUI 验收。

## 预期文件

- `crates/pinora-app/src/settings_panel.rs`
- `crates/pinora-app/src/runtime.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `crates/pinora-app/src/lib.rs`
- `AGENTS.md`、`.context/plans/041_settings_panel.md`、`.context/tasks/041_settings_panel.md`
- `.context/system/overview.md`、`.context/system/risks.md`

## 验收标准

1. 设置面板可以通过 `S` / `Ctrl+,` 打开并列出四个设置字段。
2. 上下选择字段，左右步进；主题在 System/Light/Dark 间循环。
3. Enter 保存并应用；Esc 取消；保存失败后草稿和运行时值不被部分更新。
4. 鼠标点击字段和步进按钮与键盘路径得到相同状态结果。
5. 定向测试覆盖边界、回滚和命中；workspace 质量门禁通过。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app settings_panel::tests -- --nocapture`
- `cargo test -p pinora-app runtime::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：窗口事件编排与设置写入同时发生时可能出现焦点或保存竞态；缓解：设置窗口单实例、草稿与已生效值分离，保存只在主线程提交。
- 风险：当前自绘面板不是完整平台原生控件；明确记录为待验证，不扩展平台支持声明。
- 回滚：移除设置窗口入口和运行时热应用，保留 `SettingsStore` 与旧启动读取路径。

## 完成记录

- 2026-08-02：完成 `settings_panel` 状态机、布局/命中、点阵自绘和键盘/鼠标事件；`S`/`Ctrl+,` 打开设置窗口。
- 2026-08-02：`AppRuntime::apply_settings` 在设置文件成功原子发布后更新贴图上限、默认不透明度和历史配额；历史超限先落盘 tombstone，再执行既有白名单清理。
- 验证：`cargo fmt --check`；设置面板 4/4；历史领域 6/6；runtime 11/11；`cargo check --workspace`；严格 Clippy；`cargo test --workspace`（157 通过，2 忽略）；`git diff --check`，均通过。
- 已知风险：设置窗口为 softbuffer 自绘实验适配器，系统主题、无障碍、HiDPI、焦点和真实 Windows/macOS GUI 尚未验证；历史浏览/预览/复用仍未实现。
