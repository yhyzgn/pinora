# 任务 072：Overlay 选区四边与四角调整

- 状态：已完成
- 计划：`.context/plans/072_selection_resize_handles.md`
- 规模：中
- 依赖：`.context/tasks/009_region_overlay.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/066_auxiliary_window_visibility_policy.md`、`.context/tasks/071_annotation_move_transaction.md`
- 生产行为变更：是；未标注的当前 Overlay 选区支持拖动四边和四角调整。

## 任务目标

在不新增窗口、截图或后台任务的条件下，为当前 Overlay 已确认选区增加稳定的八方向调整热区；每次拖动都保持物理像素、画布边界和最小尺寸约束，且不丢失已有标注或破坏 tray-only 生命周期。

## 范围

- 为 `SelectionSession` 增加八方向调整模型、边界 clamp 和最小尺寸保护。
- 为 Overlay 增加热区呈现、命中、拖动与释放恢复。
- 覆盖方向几何、反向/边界拖动、未标注保护、既有键盘移动和窗口策略回归。
- 更新计划、任务、系统事实和风险记录。

## 非目标

- 不实现比例锁定、旋转、磁吸、跨屏、标注对象缩放、文本编辑、贴图缩放或新的工具栏。
- 不改捕获、标注渲染、导出、OCR、贴图、历史、系统菜单、窗口策略或 worker。

## 预期文件

- `crates/pinora-core/src/selection.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `AGENTS.md`
- `.context/plans/072_selection_resize_handles.md`
- `.context/tasks/072_selection_resize_handles.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 四边与四角均可调整；所有结果在 bounds 内、至少为 `min_edge`，反向拖动不会翻转或产生非法矩形。
2. 已提交标注或草稿存在时不进入选区调整；调整不创建标注事务、不改变导出、贴图、OCR 或后台任务输入。
3. 热区与拖动只作用于现有 Overlay；Pinora 空闲仅在 tray，Overlay/贴图/辅助层禁止进入任务栏、Dock 或分页器。

## 验证

- `cargo test -p pinora-core selection -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：边缘/角落计算违反最小尺寸或 bounds。缓解：在 core 单点实现、覆盖八方向和反向/极限坐标。
- 风险：调整时重置标注文档或资产。缓解：只在文档与草稿均为空时允许进入，其他情况保持既有重选路径。
- 风险：热区让高频绘制变慢或绕过窗口策略。缓解：复用现有 Overlay 脏区/节流，不添加窗口或 worker，并执行源码守卫。
- 回滚：移除调整模型、热区与输入分支；保留既有拖选、键盘微调、标注、导出、tray 和窗口策略。

## 完成记录

- 已完成：核心新增八方向 `SelectionHandle`、边缘/角落中心与 `resize_from_handle`。调整点被限制在当前 bounds，最小边长由同一模型保护；跨过对边时停在最小尺寸，不翻转选区。
- 已完成：现有 Overlay 在未标注的 Ready 选区绘制八个白色热区，命中采用最近中心；拖动仅改变内存选区，采用既有 2 像素/32ms 重绘节流，抬起或快捷完成时才同步源选区、工具栏和资产身份。
- 已完成：有草稿或已提交标注时热区不接管输入，避免丢失标注或污染导出、OCR、贴图/后台任务输入；本任务没有新窗口、事件循环、系统菜单、截图或 worker，继续受 `window_policy` 的 tray-only 源码守卫约束。
- 已验证：`cargo test -p pinora-core selection -- --nocapture`、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`、`cargo test -p pinora-app window_policy::tests -- --nocapture`、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check` 与 `ctx validate` 通过；全量离线结果为 app 177 通过、2 忽略，core 84 通过。
- 未验证：离线测试不能证明真实 Windows、macOS、X11、KDE Wayland 中的热区可读性、HiDPI 命中、连续拖动帧时间、任务栏/Dock/分页器隔离或 tray 连续驻留；这些仍需原生会话验收。
