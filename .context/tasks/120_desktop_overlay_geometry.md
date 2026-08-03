# 任务 120：桌面 Overlay 坐标与选区命中边界

- 状态：已完成
- 计划：`.context/plans/120_desktop_overlay_geometry.md`
- 规模：中
- 依赖：任务 109、113、119 已完成。
- 生产行为变更：否；内部纯几何模块所有权迁移。

## 任务目标

把 app 内的 Overlay 像素坐标转换和选区手柄命中规则迁入 `pinora-desktop`，以离线回归测试锁定现有行为并收窄 `desktop_shell` 的职责。

## 变更前记录

```text
目的：将无窗口 Overlay 坐标规则归属到 pinora-desktop。
影响路径：desktop_shell 输入映射、选区调整、选区读数、标注局部坐标、OCR 文字拖选。
兼容性：不改变公共接口、数据形状、状态字符串、租户或权限语义。
外部副作用：无；不创建窗口、不访问文件、不启动线程、不连接外部基础设施。
回滚点：恢复 desktop_shell 内原纯函数并移除 overlay_geometry 导出。
验证场景：等比例/非等比例映射、零尺寸、倒序端点、选区内外坐标、手柄重叠与角优先级。
```

## 范围

- 新增 `crates/pinora-desktop/src/overlay_geometry.rs`。
- 迁移缓冲矩形到原图矩形、选区到标注局部坐标、窗口物理点/矩形到图像坐标的映射。
- 迁移选区可调整判定和选区手柄命中。
- 迁移既有 app 坐标/命中测试并补充退化输入测试。
- 更新 `pinora-desktop/src/lib.rs`、`desktop_shell.rs`、设计文档及 `.context/system/{overview,conventions,risks}.md`。

## 非目标

- 不迁移 `OverlayState`、`SelectionSession`、标注会话、图片采样、窗口创建、Surface 上传、winit 事件或唯一 EventLoop。
- 不改变选区工具栏、读数文案、标注渲染、OCR 结果、贴图布局或 tray-only 策略。

## 预期文件

- `AGENTS.md`
- `.context/plans/120_desktop_overlay_geometry.md`
- `.context/tasks/120_desktop_overlay_geometry.md`
- `crates/pinora-desktop/src/{lib,overlay_geometry}.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-desktop` 唯一拥有可重用的 Overlay 坐标和选区命中原语，app 删除对应重复实现。
2. 等比例/缩放映射、选区外移动、窗口零尺寸、倒序拖拽与窄选区手柄冲突均由 desktop crate 测试覆盖。
3. app 仍独占窗口、Surface、Overlay 状态机和 EventLoop；crate 依赖不新增 app/capture/jobs/softbuffer/tray。

## 验证

- `cargo test -p pinora-desktop -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：缩放取整、零尺寸处理或手柄枚举顺序变化导致选区、OCR/标注坐标偏移。
- 回滚：恢复 `desktop_shell` 内坐标/命中函数与测试，移除 desktop `overlay_geometry` 导出；不触碰窗口、输入、截图、OCR、导出或数据格式。

## 完成记录

- 代码迁移：新增 `crates/pinora-desktop/src/overlay_geometry.rs`，迁入 Overlay 缓冲/原图/窗口物理像素转换、选区局部标注映射、选区手柄命中和可调整判定；app 以 `PixelSize` 调用 crate API，并删除原有重复实现。
- 回归覆盖：desktop 新增等比例/缩放映射、零尺寸、极大图像坐标、倒序端点、选区内外局部映射和窄选区手柄测试；对应 app 测试已迁移，保留 Overlay 读数等 app 编排测试。
- 定向验证：`cargo test -p pinora-desktop -- --nocapture`，87 项通过；`cargo test -p pinora-app --lib -- --nocapture`，36 项通过；`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace` 通过，capture/export 各 1 项真实桌面测试按既有约定忽略。
- 最终门禁：`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --check`、`git diff --check` 与 `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora` 全部通过。
- 已验证事实：离线几何语义、crate 边界和 Windows 静态检查；未知项：真实 winit/窗口管理器坐标、HiDPI、连续输入、焦点、任务栏/Dock 与性能尚未验证。
