# 任务 119：桌面 XRGB 渲染原语边界

- 状态：已完成
- 计划：`.context/plans/119_desktop_xrgb_renderer.md`
- 规模：中
- 依赖：任务 109、111、113 已完成。
- 生产行为变更：否；内部纯渲染模块所有权迁移。

## 任务目标

把 app 内贴图基础帧缓存和通用 XRGB 栅格化原语迁入 `pinora-desktop`，收窄 `desktop_shell` 的呈现职责。

## 范围

- 新增 `crates/pinora-desktop/src/xrgb.rs`。
- 迁移 `PinRenderCache`、最近邻缩放、压暗、脏区恢复、矩形/选区手柄/词框/边框绘制。
- 迁移并补强像素回归测试。
- 更新 `pinora-desktop/src/lib.rs` 与 `pinora-app/src/desktop_shell.rs` 导入。
- 更新设计文档及 `.context/system/{overview,conventions,risks}.md`。

## 非目标

- 不迁移 `PinWin`、`OverlayState`、winit/softbuffer 资源或窗口创建/展示。
- 不更换渲染算法、颜色、缓冲上传策略或性能目标。

## 预期文件

- `AGENTS.md`
- `.context/plans/119_desktop_xrgb_renderer.md`
- `.context/tasks/119_desktop_xrgb_renderer.md`
- `crates/pinora-desktop/src/{lib,xrgb}.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-desktop` 唯一拥有可重用的 XRGB 渲染原语和 `PinRenderCache`；app 删除对应重复实现。
2. 缩放、压暗、缓存命中、边框裁剪和选区手柄像素行为由 desktop crate 测试覆盖。
3. app 保留唯一 EventLoop、Window/Surface 生命周期和 tray-only 策略，依赖图不新增上行依赖。

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

- 风险：像素边界、尺寸溢出和缓存键迁移遗漏导致显示回归。
- 回滚：恢复 app 内 XRGB 函数与缓存类型，移除 desktop `xrgb` 导出；不触碰窗口、输入或数据格式。

## 完成记录

- 代码迁移：新建 `crates/pinora-desktop/src/xrgb.rs`，迁入基础帧缓存及通用 XRGB 栅格化原语；`desktop_shell` 改为调用 crate API，继续独占唯一 EventLoop、Window/Surface 生命周期、Overlay/贴图状态和 tray-only 窗口策略。
- 定向验证：`cargo test -p pinora-desktop -- --nocapture`，83 项通过；`cargo test -p pinora-app --lib -- --nocapture`，41 项通过；`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace` 通过，capture/export 各 1 项真实桌面测试按既有约定忽略。
- 最终门禁：`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --check`、`git diff --check` 与 `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora` 全部通过。
- 已验证事实：纯像素结果、缓存安全边界、app 依赖方向和 Windows 交叉检查；未知项：真实图形表面、HiDPI、持续 resize、焦点、任务栏/Dock 与帧时间尚未验证。
