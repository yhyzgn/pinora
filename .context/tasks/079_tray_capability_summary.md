# 任务 079：托盘能力摘要

- 状态：已完成
- 计划：`.context/plans/079_tray_capability_summary.md`
- 规模：小
- 依赖：`.context/tasks/044_tray_actions.md`、`.context/tasks/050_tray_only_windows.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/078_tray_feedback.md`
- 生产行为变更：是；现有 tray 菜单显示本次启动的受控能力摘要。

## 任务目标

让 tray-only 桌面壳在既有菜单中展示截图、全局热键、系统图像剪贴板和本地 OCR 的只读能力状态。状态模型不泄漏 runtime notes；热键状态取自实际 `GlobalHotkeyHub` 注册，而不是仅按目标平台判断。

## 范围

- 新增可测试的 tray 能力摘要模型与固定中文标签。
- 扩展 `AppTray::try_new`，在既有菜单中附加禁用的能力标题和条目。
- 在 `desktop_shell` tray 初始化前组装已存在的运行时能力、实际热键状态和无进程 OCR 检查结果。
- 更新工作指针、项目上下文与风险登记。

## 非目标

- 不新增诊断窗口、诊断导出、权限跳转、通知、日志查看或能力热刷新。
- 不调用 OCR CLI、不会新建 worker、不会修改平台 probe、截图、热键、剪贴板、设置、历史、导出或窗口策略接口。
- 不用离线测试表述真实平台菜单、权限、任务栏/Dock/分页器已验收。

## 预期文件

- `crates/pinora-app/src/{desktop_shell.rs,lib.rs,tray.rs,tray_capabilities.rs}`
- `AGENTS.md`
- `.context/plans/079_tray_capability_summary.md`
- `.context/tasks/079_tray_capability_summary.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. tray 展示受限、脱敏的截图、实际热键、系统图像剪贴板和 OCR 状态；菜单文字不含 note、路径、OCR/剪贴板内容或原始后端错误。
2. `GlobalHotkeyHub` 注册失败时，tray 标明热键受限但不影响截图菜单、IPC 或 tray 驻留；成功时显示可用。
3. 能力条目只使用现有 tray 菜单，不创建窗口、事件循环、worker、外部进程、通知或网络活动；窗口策略守卫继续通过。

## 验证

- `cargo test -p pinora-app tray_capabilities -- --nocapture`
- `cargo test -p pinora-app tray::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：tray 后端不展示或延迟刷新禁用能力项。缓解：不依赖其控制业务，保留既有 tray 手动入口和 IPC；真实桌面验证单独记录。
- 风险：启动快照在外部依赖变更后陈旧。缓解：明确不实现热刷新；下次启动重新探测。
- 风险：热键可用性与目标平台不一致。缓解：以 `GlobalHotkeyHub::status().available` 为唯一菜单热键状态来源。
- 回滚：移除摘要模型、菜单项和启动传参；不影响截图、OCR、剪贴板、热键、tray 生命周期和窗口隔离。

## 完成记录

- 新增 `tray_capabilities`，以四个布尔快照产生固定、脱敏的 tray 标签；测试确认实际
  热键结果会覆盖 runtime 的平台猜测，且 runtime notes 不会显示。
- `AppTray` 仅添加禁用信息项；`desktop_shell` 在创建 tray 前传入既有运行时截图/剪贴板
  状态、实际热键注册状态和无进程 OCR PATH 检查结果。
- 2026-08-02 验证通过：能力摘要/tray/桌面状态机/窗口策略定向测试、格式、workspace
  编译、严格 Clippy、离线全量测试（app 204 通过、2 项真实桌面测试忽略；core 85
  通过）、差异检查和上下文校验。
- 剩余风险：禁用菜单项在原生 tray 中的可见性、读屏语义、权限实况、窗口管理器的
  taskbar/Dock/分页器隔离尚未自动化；不将本任务的离线证据表述为平台验收。
