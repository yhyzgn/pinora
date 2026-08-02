# 任务 075：贴图关闭撤销

- 状态：进行中
- 计划：`.context/plans/075_restore_closed_pin.md`
- 规模：中
- 依赖：`.context/tasks/044_tray_management.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/066_auxiliary_window_visibility_policy.md`
- 生产行为变更：是；tray 可恢复最近关闭的贴图。

## 任务目标

给 tray 增加“撤销关闭贴图”动作，恢复单个最近关闭贴图的图像和显示变换。快照不能含旧窗口/任务句柄；恢复只能经既有 pin 创建与 `window_policy`，使用新的领域身份。

## 范围

- 增加纯最近关闭快照与恢复资格规则。
- 接入贴图关闭、tray 动作、恢复创建与失败保留。
- 覆盖新身份、变换、空/失败恢复和窗口策略回归。
- 更新计划、任务、系统事实和风险登记。

## 非目标

- 不实现多级撤销、持久化、OCR/导出任务恢复、动画或新窗口类型。

## 预期文件

- `crates/pinora-app/src/desktop_shell.rs`
- `crates/pinora-app/src/tray.rs`
- `AGENTS.md`
- `.context/plans/075_restore_closed_pin.md`
- `.context/tasks/075_restore_closed_pin.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 关闭最近贴图后 tray 可恢复其图像、位置、缩放、透明度、锁定和置顶；恢复使用新的 PinId/asset/window，不接收旧 owner 结果。
2. 空快照不执行操作；创建失败不消费快照，允许再次恢复；恢复成功只消费一次。
3. 除用户显式恢复时的既有 Pin 外不创建窗口，所有辅助窗口继续隐藏创建、唯一展示，空闲仅 tray。

## 验证

- `cargo test -p pinora-app tray -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：迟到任务污染恢复贴图。缓解：关闭旧 owner，恢复新 identity，不复制任务/OCR。
- 风险：恢复创建失败丢失数据。缓解：仅创建成功后消费内存快照。
- 风险：恢复窗口绕过 tray-only 边界。缓解：只经既有 `spawn_pin`/`window_policy`，执行源码守卫。
- 回滚：删除 tray 恢复动作和快照；恢复原关闭语义，不影响现有贴图、截图、导出、OCR 或 tray。

## 完成记录

- 已完成：`ClosedPinSnapshot` 仅保存不可变图像和呈现/领域值；关闭旧贴图时先关闭 OCR/导出 owner 与 runtime Pin，再保存最近快照并启用 tray 撤销。
- 已完成：tray 恢复使用新的 `PinId`、asset 和 `spawn_pin` 创建受 `window_policy` 隐藏创建、唯一展示的贴图窗口；恢复成功后保留位置、缩放、透明度、锁定和置顶。创建失败关闭新 runtime Pin，最近快照保留以便重试；成功仅可恢复一次。
- 已完成：不新增窗口类型、事件循环、截图、系统菜单或 worker；恢复只是用户显式 tray 动作下复用既有 Pin 窗口工厂，空闲继续只驻留 tray。
- 已验证：`cargo test -p pinora-app tray -- --nocapture`、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check` 与 `ctx validate` 通过；全量离线结果为 app 183 通过、2 忽略，core 85 通过。
- 未验证：离线测试不能证明真实 Windows、macOS、X11、KDE Wayland 中 tray 动作、恢复首帧、HiDPI、焦点、任务栏/Dock/分页器隔离或连续恢复性能；仍需原生会话验收。
