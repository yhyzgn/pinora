# 计划 103：Wayland Portal 版本门槛

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/103_wayland_portal_version_gate.md`

## 目标

让 Wayland `GlobalShortcuts` Portal 适配器只在支持当前 session/bind/signal 调用链的 v2 接口上运行；旧版本在连接后立即进入稳定的版本不支持状态，避免误报可用再失败。

## 非目标

- 不改变 Portal shortcut ID、授权 UI、后台线程、GUI 轮询、tray/IPC 回退或设置 schema。
- 不实现 v1 兼容分支、ConfigureShortcuts、新窗口或任意输入监听。
- 不把版本字符串、D-Bus 原始错误或会话对象路径写入用户可见诊断。

## 约束

- 当前实现依赖 XDG `GlobalShortcuts` v2 的固定 session/bind/Activated 契约，最低版本必须与实现一致。
- 版本不足只能发布 `portal_version_unsupported`，不得创建绑定或报告 Available。

## 依赖关系

- 依赖任务 100 的 `wayland_portal` worker、稳定 `PortalFailure` 和能力状态接线。
- 依赖官方 XDG Desktop Portal `GlobalShortcuts` 接口版本定义。

## 检查点

1. v2 及以上允许进入 CreateSession/BindShortcuts。
2. v0/v1 在任何 D-Bus 请求前返回 VersionUnsupported。
3. 版本门槛测试与全量门禁通过，其他平台条件编译不受影响。

## 阶段

1. 更新版本常量和纯契约测试。
2. 更新任务/system/risk 记录并执行 workspace、跨 target 和上下文门禁。
3. 提交并推送。

## 计划级风险

- 某些 backend 可能报告较高版本但只实现部分方法，仍需保留现有方法失败的稳定降级。
- 真实 KDE/GNOME 授权与信号行为不由版本门槛证明，继续由 R-059 跟踪。

## 完成标准

- Portal 适配器不会在 v1 或更低版本上尝试绑定；v2 门槛和降级状态有测试证据。
- 定向、workspace、Clippy、全量测试、Windows target、`ctx validate` 和差异检查通过。

## 风险与回滚

- 若目标环境证明 v1 也完整支持当前调用链，可在独立任务中降低门槛并补官方/实机证据。
- 回滚仅恢复旧常量，不改变 tray/IPC 或其他平台热键。

## 完成记录

- 2026-08-03：将 `PORTAL_MIN_VERSION` 收紧为 v2，v0/v1 在创建 session 前稳定降级为 `portal_version_unsupported`；新增版本契约测试。
- 2026-08-03：`cargo fmt --check`、Portal 定向测试、workspace 测试、`cargo check --workspace`、Clippy、Windows target 检查、`ctx validate`、`git diff --check` 均通过。
- 2026-08-03：已提交并推送任务 103；真实 Wayland 授权、Activated 信号、tray-only 和性能证据继续由 R-059/R-062 跟踪。
