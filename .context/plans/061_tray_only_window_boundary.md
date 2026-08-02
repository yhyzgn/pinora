# 计划 061：Tray-only 窗口边界

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/061_tray_only_window_boundary.md`

## 目标

把 Pinora 的 GUI 生命周期收敛为单一受托盘监督的 `desktop_shell`：空闲进程仅有 tray、热键、IPC 与帧缓存；Overlay、贴图、设置和历史只能作为该会话中的辅助窗口出现，绝不通过公开旁路启动无托盘事件循环或产生额外任务栏/Dock 项。

## 非目标

- 不声称离线测试可以确认 Windows、macOS、X11 或 Wayland 的实际任务栏、Dock、分页器和合成器行为。
- 不重写截图、OCR、导出、标注或贴图交互；不引入新的 GUI 框架或平台依赖。

## 约束

- 空闲常驻进程只能通过系统 tray、已成功注册的热键、单实例 IPC 和帧缓存提供入口；没有控制窗口或独立 GUI 会话。
- Overlay、贴图、设置和历史只能由已成功创建 tray 的 `run_desktop_shell` 会话创建，且必须经 `window_policy` 应用平台任务栏/Dock 隔离请求。
- 删除的公开 GUI API 不以兼容包装器保留，因为任何包装器都不得重新创建不受 tray 监督的事件循环。

## 依赖关系

- 依赖 050/054 的无控制窗口启动与 `window_policy` 平台隔离策略。
- 依赖 058 的捕获失败返回 tray 状态机，以及 060 的客户区菜单和编辑 Overlay。
- 依赖当前 `pinora-app` 只由根二进制引用的事实；移除未被仓库调用的遗留公开 GUI 入口前必须检索全部引用。

## 阶段

1. 审计公开 GUI API 与事件循环/窗口创建点，识别可绕过 `run_desktop_shell` 的旁路。
2. 删除或封闭无托盘独立 Overlay、区域工作流和贴图会话入口，保留桌面壳所需的纯布局计算。
3. 增加源代码级架构门禁，验证事件循环与 `create_window` 只能位于 `window_policy` 边界；运行严格门禁并记录真实桌面缺口。

## 检查点

1. `pinora-app` 不再公开可创建无 tray GUI 会话的函数或类型。
2. 所有生产窗口仍经 `window_policy::create_auxiliary_window` 创建；唯一事件循环构造集中在 `window_policy`。
3. 现有 tray 启动、Overlay、贴图、菜单和编辑路径保持编译与离线测试覆盖。

## 计划级风险

- 此调整删除未稳定的公开 API，潜在第三方使用者需要迁移到受托盘监督的二进制流程；仓库内引用为零不代表外部依赖为零。
- 源代码门禁只能防止本仓库的结构性回归，不能证明操作系统最终不会显示任务栏/Dock 项。
- 真实 GUI 进程和合成器行为必须在平台会话中探针，CI 只作为静态/单元验证。

## 变更前记录

```text
目的：确保 Pinora 进程自启动到退出只以 tray 为后台常驻入口，任何可见 UI 都是受管辅助窗口。
影响路径：pinora-app 公共导出、遗留独立 GUI 模块、window_policy 架构测试、上下文文档。
兼容性：删除未受托盘监督的公开 GUI API；不改变截图数据、贴图 PinId、状态字符串、权限或持久化格式。
外部副作用：无网络、无共享服务；仅影响本机窗口创建的可达路径。
回滚点：恢复遗留导出会重新暴露无 tray GUI 旁路，因此只能与替代的受监督 API 一同恢复。
验证场景：仓库引用检索、窗口/事件循环源代码门禁、workspace 构建、严格 Clippy、全量离线测试与上下文校验。
```

## 完成标准

- 不存在可由 `pinora-app` 公开 API 创建无系统托盘 GUI 会话的路径。
- 生产源代码只由 `window_policy` 调用 `EventLoop::builder` 与 `ActiveEventLoop::create_window`。
- 本地严格门禁和 `ctx validate` 通过；任务栏/Dock 的真实平台验证缺口明确记录。

## 完成记录

- 已完成：移除 `run_pin_session`、`PinView`、`PinSessionEnd`、`run_region_selection` 与 `capture_region_interactive`，以及三条专属独立事件循环实现；仓库内引用检索为零。
- 已完成：将纯 `scaled_window_size` 移入不持有窗口资源的 `pin_layout`，`desktop_shell` 保持复用；唯一公开 GUI 会话入口为 `run_desktop_shell`。
- 已完成：`window_policy` 源代码门禁会扫描应用模块，拒绝该模块之外的 `EventLoop::builder` 或直接 `.create_window(` 调用。
- 已验证：窗口策略定向测试、fmt、workspace check、严格 Clippy、全量离线测试、diff 检查与 `ctx validate` 通过。
- 未验证：真实 Windows/macOS/X11/KDE Wayland 的任务栏、Dock、分页器、合成器映射、托盘连续驻留与输入性能仍需平台 GUI 探针。
