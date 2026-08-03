# 任务 128：辅助面板窗口适配 crate

- 状态：已完成
- 计划：`.context/plans/128_panels_crate.md`
- 规模：中
- 依赖：任务 109、111、112、113、127 已完成。
- 生产行为变更：否；三个辅助面板窗口适配的内部所有权迁移。

## 任务目标

让 `pinora-panels` 唯一拥有设置、历史、诊断窗口的 Window/Surface、Panel 状态和绘制适配，
让 `pinora-app` 继续独占唯一 EventLoop、业务编排和用户可见副作用。

## 变更前记录

```text
目的：将 app 内三个面板窗口适配器迁入独立功能 crate，修复窗口资源和业务编排混在 app 的边界。
影响路径：设置、历史、诊断三个辅助窗口的创建、输入转发、主题刷新、重绘、resize 和关闭。
兼容性：窗口标题、尺寸、窗口策略、面板状态、设置/历史/诊断格式、状态字符串和权限语义不变。
外部副作用：仍只创建用户主动打开的既有辅助窗口；继续使用唯一 EventLoop 和既有 window_policy。
回滚点：恢复三个 app 内窗口模块和导入，移除 pinora-panels workspace 成员。
验证场景：三个面板的编译调用、window_policy 源码守卫、主题/面板回归、workspace 和 Windows target。
```

## 范围

- 新增 `crates/pinora-panels/{Cargo.toml,src/lib.rs,src/settings_window.rs,src/history_window.rs,src/diagnostics_window.rs}`。
- 从 `pinora-app/src` 迁移三个窗口适配器，公开跨 crate 所需的最小方法集合。
- 更新 workspace、app 依赖/import、设计文档、overview、conventions 和 risks。

## 非目标

- 不迁移 `DesktopApp`、`ApplicationHandler`、Overlay/贴图窗口、托盘、截图、OCR、导出、历史
  业务策略或诊断报告模型。
- 不新增依赖、原始 SQL、警告抑制、网络访问或真实 GUI 测试。

## 预期文件

- `AGENTS.md`
- `.context/plans/128_panels_crate.md`
- `.context/tasks/128_panels_crate.md`
- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-panels/{Cargo.toml,src/*.rs}`
- `crates/pinora-app/Cargo.toml`
- `crates/pinora-app/src/{lib,desktop_shell,settings_window,history_window,diagnostics_window}.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-panels` 独立编译，且不依赖 app、tray、capture、ocr、export、runtime 或外部进程。
2. 三个窗口适配器只通过 `window_policy` 创建/展示，仍使用同一面板状态和 softbuffer 绘制路径。
3. app 继续拥有唯一 EventLoop 和业务副作用，所有既有面板调用点编译并回归通过。
4. workspace 测试、check、严格 Clippy、Windows target、fmt、diff 与 ctx validate 通过。

## 验证

- `cargo test -p pinora-panels -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo run --quiet -- --version`
- `cargo fmt --all -- --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：适配器迁移时丢失 `window_policy` 调用、主题刷新、surface resize 或 app 的事件转发。
- 回滚：恢复三个 app 内窗口模块和原依赖，移除 `pinora-panels`；不触碰面板绘制、设置 schema、
  历史索引、诊断报告、截图、贴图或托盘生命周期。

## 完成记录

- 2026-08-03 已完成。新增 `pinora-panels` crate，迁移三个辅助面板窗口适配器并将其跨 crate
  所需 API 明确为公开接口；删除 app 内三个窗口模块，`desktop_shell` 改为直接消费新 crate。
  新 crate 只依赖 core、desktop、storage、softbuffer 和 winit，不依赖 app/tray/capture/ocr/
  export/runtime；源码测试守卫直接拒绝绕过 `window_policy` 的窗口创建和显示调用。
- 验证通过：`cargo test -p pinora-panels -- --nocapture`（1 项）、
  `cargo test -p pinora-app --lib -- --nocapture`（22 项）、
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、
  `cargo run --quiet -- --version`（输出 `pinora 0.1.0`）、`cargo fmt --all -- --check`、
  `git diff --check` 与 `ctx validate`。
- 未覆盖风险：未连接真实共享基础设施；也未验证原生窗口创建、surface 首帧、主题事件、焦点、
  任务栏/Dock/分页器、HiDPI、真实文件权限或性能；由 R-079 继续跟踪。
