# 任务 139：Overlay 窗口适配器

- 状态：已完成
- 计划：`.context/plans/139_overlay_window_adapter.md`
- 规模：中
- 依赖：任务 111、128、133、134、138 已完成。
- 生产行为变更：否；Overlay Window/Surface 资源所有权迁移。

## 任务目标

让 `pinora-panels::OverlayWindow` 唯一持有当前 Overlay 的 `Window` 与 `Surface`，并通过统一
`window_policy` 执行隐藏创建、展示、焦点、IME、隐藏和固定像素表面 resize；app 仍持有 Overlay 会话状态与
所有业务副作用。

## 变更前记录

```text
目的：将 Overlay 平台资源从 desktop_shell 的会话状态中抽出，沿用既有 panels 窗口适配边界并缩小 app 的窗口资源职责。
影响路径：区域/全屏/虚拟桌面/窗口/历史编辑/贴图编辑 Overlay 的创建、展示、焦点、IME、关闭和 Surface 尺寸同步。
兼容性：OverlayPresentation、标题、尺寸、位置、装饰、可调整、置顶、窗口策略、选择/标注/导出/OCR/任务 owner 和状态字符串不变。
外部副作用：无新增；仍由既有 window_policy 创建和展示同一个 Overlay，不创建额外窗口，不调用网络或外部基础设施。
回滚点：删除 pinora-panels::OverlayWindow 并恢复 OverlayState 的 Window/Surface 字段。
验证场景：资源创建/展示源码守卫、当前 Overlay ID 路由、关闭隐藏、固定 XRGB 尺寸同步、panels/app/workspace 回归和依赖图。
```

## 范围

- 在 `crates/pinora-panels/src/` 新增 Overlay Window/Surface 适配器及源码边界回归测试。
- 替换 `OverlayState` 的直接资源字段和所有资源访问点。
- 更新 crate 导出、设计文档、系统事实、风险和工作指针。

## 非目标

- 不迁移 Overlay 会话、标注、绘制、输入、预览缓存、贴图窗口、任务、导出、OCR、tray 或 runtime。
- 不新增依赖、原始 SQL、警告抑制、网络访问或真实 GUI 测试。

## 预期文件

- `AGENTS.md`
- `crates/pinora-panels/src/{lib,overlay_window}.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `.context/{plans,tasks}/139_overlay_window_adapter.md`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `OverlayWindow` 唯一持有 Overlay Window/Surface，且所有创建/展示均经 `window_policy`；app 不再直接字段持有这些资源。
2. Overlay 的所有展示模式、关闭 owner、焦点、IME 和固定 XRGB 表面尺寸语义不变，不创建额外窗口。
3. panels 生产依赖不含 app、capture、jobs、export、history、ocr、tray 或 runtime；定向与全量门禁通过。

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

- 风险：平台资源代理可能影响 Overlay 映射、首帧、焦点、IME 或 resize 时序。
- 回滚：删除 `OverlayWindow` 并恢复 `OverlayState` 的直接资源字段；不触碰选区、标注、任务、导出、OCR、tray 或设置。

## 完成记录

- 已完成：新增 `pinora-panels::OverlayWindow`，封装 Overlay 的隐藏创建、`Surface` 初始化、展示、隐藏、焦点、
  IME、重绘和原始像素尺寸同步。`OverlayState` 删除直接 `Rc<Window>`/`Surface` 字段，仅持有适配器；7 个
  既有资源访问点均改由适配器路由，Overlay 会话、绘制、输入、任务、关闭 owner 和 EventLoop 未迁移。
- 兼容性：五种 `OverlayPresentation` 的标题、尺寸、位置、全屏/装饰/可调整/置顶、IME、焦点、隐藏创建和展示顺序
  保持不变；不创建额外窗口，不改变选择、标注、导出、OCR、tray 或状态字符串。
- 已验证：`cargo test -p pinora-panels -- --nocapture`（1 通过）、`cargo test -p pinora-app --lib -- --nocapture`
  （11 通过）、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、
  `cargo run --quiet -- --version`、`cargo fmt --all -- --check`、`git diff --check`、
  `cargo tree -p pinora-panels -e normal --depth 1` 与 `ctx validate` 均通过。
- 风险与回滚：真实任务栏/Dock/分页器、tray-only、首帧、焦点、IME、HiDPI 与连续拖拽帧时间仍未覆盖，R-085
  持续跟踪；回滚时删除 `OverlayWindow` 并恢复 `OverlayState` 的直接资源字段，不触碰选区、标注、任务、导出、
  OCR、tray 或设置。
