# 任务 067：Overlay 区域模糊标注

- 状态：已完成
- 计划：`.context/plans/067_blur_annotation.md`
- 规模：中
- 依赖：`.context/tasks/030_annotation_revision_contract.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/066_auxiliary_window_visibility_policy.md`
- 生产行为变更：是；Overlay 支持可撤销的局部区域模糊。

## 任务目标

在当前 Overlay 内增加 `B` 与工具栏入口的区域模糊。每个有效拖拽提交冻结半径，从原始截图采样并生成确定性局部盒模糊；预览和最终烧录严格一致，且不得破坏 tray-only 或任务栏/Dock 隔离边界。

## 范围

- 为 `Annotation`、`DraftShape` 和 `AnnotateSession` 增加 Blur 工具、半径冻结和提交语义。
- 使用分离滑动盒模糊渲染 Blur，接入预览与最终烧录。
- 增加工具栏按钮、`B` 快捷键和核心/工具栏/窗口策略回归。
- 更新计划、任务、系统事实与风险记录。

## 非目标

- 不增加高斯模糊、强度配置、GPU、模糊笔刷、已有对象编辑、持久化偏好或新窗口。
- 不改变截图、贴图、OCR、导出、任务监督、公共 IPC、持久化形状或窗口工厂。

## 预期文件

- `crates/pinora-core/src/{annotate.rs,lib.rs}`
- `crates/pinora-app/src/{desktop_shell.rs,overlay_toolbar.rs,window_policy.rs}`
- `AGENTS.md`
- `.context/plans/067_blur_annotation.md`
- `.context/tasks/067_blur_annotation.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. `B` 和工具栏选择 Blur；退化拖拽不产生事务，提交对象冻结安全半径。
2. 梯度图中模糊区域像素变化，区域外字节保持不变；反向与边界选区安全。
3. 草稿预览与同一已提交 Blur 的烧录输出逐字节一致；撤销/重做沿用文档事务。
4. 工具栏命中、窄画布换行和 `window_policy` 递归源码守卫持续通过，未创建新窗口或任务。
5. 定向与全量离线门禁通过；真实 4K/HiDPI 帧时间、任务栏/Dock、tray、焦点和合成器验证缺口明确保留。

## 验证

- `cargo test -p pinora-core annotate -- --nocapture`
- `cargo test -p pinora-app overlay_toolbar::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：大范围模糊预览增加主线程 CPU 和临时内存。缓解：固定半径、两遍滑动和只保存有限采样边框；真实帧时间列为开放风险。
- 风险：边界重复采样或整数平均导致跨平台输出不稳定。缓解：锁定整数求和、除法和坐标钳制，使用梯度/边界回归。
- 风险：入口绕过窗口策略。缓解：只扩展现有 Overlay 状态机并运行递归源码守卫。
- 回滚：移除 Blur 字段、栅格化和入口；既有标注、tray 与窗口策略不变。

## 完成记录

- 已在 `pinora-core` 增加区域 Blur 标注模型、草稿事务、提交半径冻结及从原始截图采样的分离滑动盒栅格化；预览和提交后烧录逐字节一致。
- 已在 Overlay 工具栏和键盘状态机增加 `B` 入口；未触碰 `window_policy` 工厂或可见映射路径，因此不新增任务栏/Dock、tray、窗口、截图或 worker 行为。
- 已新增退化拖拽、冻结半径、区域内变化/区域外不变、反向边界和超大半径钳制回归；工具栏换行/命中与递归窗口策略守卫持续通过。
- 2026-08-02 验证通过：`cargo test -p pinora-core annotate -- --nocapture`、`cargo test -p pinora-app overlay_toolbar::tests -- --nocapture`、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`、`cargo test -p pinora-app window_policy::tests -- --nocapture`、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check`、`ctx validate`。
- 未验证：真实 4K/HiDPI 连续拖拽帧时间和各原生窗口管理器的 tray、任务栏/Dock、焦点及合成器表现；这些未作为本任务完成证据。
