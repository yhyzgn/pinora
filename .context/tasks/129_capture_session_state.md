# 任务 129：捕获会话状态模块

- 状态：已完成
- 计划：`.context/plans/129_capture_session_state.md`
- 规模：中
- 依赖：任务 122、124、125、126、127、128 已完成。
- 生产行为变更：否；捕获会话状态和目标映射的内部模块迁移。

## 任务目标

让 `pinora-app::capture_session` 唯一拥有捕获模式、延时状态、失败范围和 Overlay 目标映射，
让 `desktop_shell` 继续独占真实捕获、窗口、线程、EventLoop、托盘反馈和恢复副作用。

## 变更前记录

```text
目的：从 desktop_shell 抽出无窗口副作用的捕获会话值对象，降低唯一事件循环文件的职责密度。
影响路径：启动截图、延时截图、窗口/历史/贴图编辑 Overlay 的目标准备和失败恢复范围判定。
兼容性：模式、初始选区、display/origin、尺寸、最小边长、编辑 PinId、延时截止和错误范围语义不变。
外部副作用：无新增；捕获后端、线程、窗口、tray、历史、导出、OCR 和 runtime 行为保持原路径。
回滚点：恢复 desktop_shell 内类型/函数，移除 capture_session 模块及上下文记录。
验证场景：标准/窗口/延时失败范围，延时截止，历史/窗口/贴图编辑目标、虚拟桌面目标和全量回归。
```

## 范围

- 新增 `crates/pinora-app/src/capture_session.rs`，迁移捕获会话值对象、目标构造和状态测试。
- 更新 `crates/pinora-app/src/{lib,desktop_shell}.rs`，删除重复定义并保持现有副作用调用。
- 更新 `AGENTS.md`、计划/任务、设计文档、overview、conventions 和 risks。

## 非目标

- 不迁移真实捕获后端、预截帧、Window/Surface、Overlay/贴图绘制、线程、EventLoop、tray 或任务服务。
- 不新增依赖、原始 SQL、警告抑制、网络访问或真实 GUI 测试。

## 预期文件

- `AGENTS.md`
- `.context/plans/129_capture_session_state.md`
- `.context/tasks/129_capture_session_state.md`
- `crates/pinora-app/src/{capture_session,lib,desktop_shell}.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `capture_session` 唯一拥有目标/模式/延时/失败范围状态，且不创建窗口、不启动线程、不调用后端。
2. `desktop_shell` 继续保留所有真实捕获、窗口、EventLoop、tray 和恢复副作用，现有用户语义不变。
3. 状态模块边界测试和 app 回归通过；workspace、Clippy、Windows target、fmt、diff 与 ctx validate 通过。

## 验证

- `cargo test -p pinora-app capture_session -- --nocapture`
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

- 风险：类型迁移遗漏导致错误恢复范围、目标坐标或编辑 PinId 改变。
- 回滚：恢复 desktop_shell 内定义并删除 capture_session；不触碰捕获后端、图像、历史、窗口、托盘或设置。

## 完成记录

- 实现：新增 `crates/pinora-app/src/capture_session.rs`，迁移 `Mode`、`CaptureFailureScope`、
  `LoadingState`、`DelayedCapture`、`OverlayPresentation`、`OverlayTarget` 与全部目标构造。
  延时快照从 winit `WindowId` 收敛为领域 `PinId`；`desktop_shell` 只在实际显示/隐藏时再映射窗口，
  因此新模块不依赖 winit。
- 兼容性：区域、全屏、全部显示器、窗口、历史编辑和贴图编辑的 display/origin、尺寸、初始选区、
  最小边长、编辑 `PinId`、延时截止和失败优先级保持原值；真实捕获、线程、窗口、tray 和恢复副作用
  未改变。
- 验证：`cargo test -p pinora-app capture_session -- --nocapture`（6 通过）；
  `cargo test -p pinora-app --lib -- --nocapture`（22 通过）；
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（通过，2 个真实桌面测试按既有条件忽略）；
  `cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo run --quiet -- --version`、
  `cargo fmt --all -- --check`、`git diff --check` 与 `ctx validate` 均通过。
- 风险与回滚：R-080 仍跟踪真实捕获、窗口管理器、焦点、HiDPI、tray-only 和性能；回滚时恢复
  `desktop_shell` 内定义并移除 `capture_session`，不触碰后端、图像、历史、窗口、托盘或设置。
