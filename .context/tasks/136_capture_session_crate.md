# 任务 136：捕获会话 crate

- 状态：已完成
- 计划：`.context/plans/136_capture_session_crate.md`
- 规模：小
- 依赖：任务 106、122、124、129、135 已完成。
- 生产行为变更：否；捕获会话纯契约的 crate 边界迁移。

## 任务目标

让 `pinora-capture` 唯一承载 `CaptureSessionMode`、`LoadingState`、`DelayedCapture`、
`CaptureFailureScope`、`OverlayPresentation`、`OverlayTarget` 及其构造/判定函数；让
`pinora-app` 只消费该 crate，不改变桌面壳的真实副作用。

## 变更前记录

```text
目的：将稳定的捕获会话值对象从 app 私有模块提升到既有捕获功能 crate，降低 desktop_shell 与 app 的职责密度。
影响路径：区域/全屏/全部显示器/窗口/历史/贴图编辑的 Overlay 目标准备、延时截图和失败恢复范围判定。
兼容性：模式、初始选区、display/origin、尺寸、最小边长、编辑 PinId、延时截止和错误范围语义不变。
外部副作用：无新增；CaptureProvider、线程、FrameCache、Window/Surface、tray、历史、导出、OCR、runtime 和 EventLoop 保持原路径。
回滚点：移除 pinora-capture::capture_session 并恢复 pinora-app::capture_session。
验证场景：标准/窗口/延时失败范围、延时截止、历史/窗口/贴图编辑目标、虚拟桌面目标、依赖图与全量回归。
```

## 范围

- 在 `crates/pinora-capture/src/` 新增捕获会话模块，迁移六项纯逻辑回归测试。
- 更新 `pinora-capture` 公开导出、`pinora-app` 模块声明和 `desktop_shell` 导入。
- 更新 `AGENTS.md`、计划/任务、设计文档与 `.context/system/`。

## 非目标

- 不迁移真实截图后端、FrameCache、CaptureProvider 调用、线程、Window/Surface、Overlay/贴图绘制、
  EventLoop、tray 或任务服务。
- 不新增依赖、原始 SQL、警告抑制、网络访问或真实 GUI 测试。

## 预期文件

- `AGENTS.md`
- `crates/pinora-capture/src/{lib,capture_session}.rs`
- `crates/pinora-app/src/{lib,desktop_shell}.rs`
- `.context/{plans,tasks}/136_capture_session_crate.md`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-capture` 唯一拥有迁移的会话类型、构造/判定函数与六项回归测试，新增模块不依赖 app、desktop 或 winit。
2. `pinora-app` 不再有 `capture_session` 内部模块；shell 的实际捕获、线程、窗口、tray 和恢复副作用不变。
3. 定向测试、workspace、Clippy、Windows target、fmt、diff 与 ctx validate 通过。

## 验证

- `cargo test -p pinora-capture -- --nocapture`
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

- 风险：可见性或命名迁移可能改变失败恢复范围、Overlay 目标坐标或延时贴图恢复。
- 回滚：删除 `pinora-capture::capture_session` 并恢复 app 私有模块；不触碰捕获后端、图像、历史、
  窗口、tray、OCR、导出或设置。

## 完成记录

- 已完成：在 `pinora-capture` 新增 `capture_session` 模块，迁移 `CaptureSessionMode`、
  `LoadingState`、`DelayedCapture`、`CaptureFailureScope`、`OverlayPresentation`、`OverlayTarget` 与
  全部构造/判定函数及 6 项回归测试。app 私有 `capture_session` 已删除，`desktop_shell` 改为直接导入
  crate 契约；真实捕获、线程、FrameCache、窗口、tray、恢复副作用和 EventLoop 未迁移。
- 兼容性：区域、全屏、全部显示器、窗口、历史编辑和贴图编辑的 display/origin、尺寸、初始选区、
  最小边长、编辑 `PinId`、延时截止和失败优先级保持原值；`CaptureSessionMode` 仅替换原 app 私有的
  泛化 `Mode` 名称。
- 已验证：`cargo test -p pinora-capture -- --nocapture`（39 通过，1 项真实显示会话忽略）、
  `cargo test -p pinora-app --lib -- --nocapture`（15 通过）、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、
  `cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo run --quiet -- --version`、
  `cargo fmt --all -- --check`、`git diff --check`、`cargo metadata --no-deps --format-version 1` 与
  `ctx validate` 均通过。
- 风险与回滚：R-080 继续跟踪真实捕获、窗口管理器、tray-only、任务栏/Dock、焦点、HiDPI 与性能；
  回滚时移除 `pinora-capture::capture_session` 并恢复 app 私有模块，不触碰捕获后端、图像、历史、窗口、
  tray、OCR、导出或设置。
