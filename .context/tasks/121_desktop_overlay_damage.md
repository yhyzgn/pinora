# 任务 121：桌面 Overlay 标注投影与脏区原语

- 状态：已完成
- 计划：`.context/plans/121_desktop_overlay_damage.md`
- 规模：中
- 依赖：任务 109、119、120 已完成。
- 生产行为变更：否；内部纯渲染辅助函数所有权迁移。

## 任务目标

将 Overlay 标注选中框投影、脏区扩张和 XRGB 块拷贝统一归属到 `pinora-desktop`，降低 `desktop_shell` 的无窗口像素逻辑。

## 变更前记录

```text
目的：将不持有窗口的标注投影和脏区像素原语归属到 pinora-desktop。
影响路径：Overlay 选中标注框、脏区恢复、标注预览 XRGB 叠加。
兼容性：不改变接口、数据、状态、租户或权限语义。
外部副作用：无；不创建窗口、不访问文件、不启动线程、不连接外部基础设施。
回滚点：恢复 desktop_shell 内纯函数并移除 desktop 对应导出。
验证场景：缩放投影、最小一像素边框、零尺寸、越界脏区、短 XRGB 源缓冲。
```

## 范围

- 新增 `crates/pinora-desktop/src/overlay_annotation.rs`。
- 迁移标注局部选中框到显示选区的投影与脏区裁剪。
- 将 XRGB 块拷贝迁入 `xrgb`。
- 迁移并补强相关像素回归测试。
- 更新 crate 导出、app 调用、设计/系统/风险文档。

## 非目标

- 不迁移标注文档、RGBA 合成、`OverlayPreviewCache` 生命周期、Window/Surface、softbuffer damage 适配或 EventLoop。
- 不改变标注工具、用户输入、截图、OCR、导出、托盘或窗口策略。

## 预期文件

- `AGENTS.md`
- `.context/plans/121_desktop_overlay_damage.md`
- `.context/tasks/121_desktop_overlay_damage.md`
- `crates/pinora-desktop/src/{lib,overlay_annotation,xrgb}.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-desktop` 唯一拥有标注显示投影、脏区裁剪和受界 XRGB 块拷贝；app 删除重复函数。
2. desktop 覆盖缩放、零尺寸、裁剪和短源缓冲，且不新增上行依赖。
3. app 仍独占标注状态、缓存、Window/Surface、softbuffer present 和唯一 EventLoop。

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

- 风险：投影取整或块拷贝边界变化会导致选中框、脏区或标注预览出现像素回归。
- 回滚：恢复 app 内投影/裁剪/块拷贝函数，移除 desktop 导出；不触碰窗口、输入、截图、OCR、导出或数据格式。

## 完成记录

- 2026-08-03：`overlay_annotation` 接管缩放投影与脏区裁剪，`xrgb::blit_xrgb_block` 接管受界块拷贝；app 已删除本地副本，未改变任何窗口、`Surface`、事件循环、标注文档、缓存或用户输入所有权。
- 测试覆盖：缩放与最小一物理像素选中框、零尺寸、越界源框、脏区裁剪/溢出边界、目标帧裁剪及短源缓冲拒绝。
- 完成验证：`cargo test -p pinora-desktop -- --nocapture`（91 通过）、`cargo test -p pinora-app --lib -- --nocapture`（36 通过）、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（capture/export 各 1 项真实桌面测试按既有约定忽略）、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --check`、`git diff --check`、`python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`。
- 未覆盖风险：离线验证不能证明真实 `softbuffer`、HiDPI、连续拖动、焦点、任务栏/Dock、托盘常驻或性能；详见 R-072。
