# 计划 085：诊断与受限能力反馈

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/085_diagnostics_capability_feedback.md`

## 目标

在不破坏 tray-only 常驻边界的前提下，补齐可由用户主动打开的本地诊断面板。面板只显示安全、有限的能力状态、稳定错误码和确定的恢复建议，使截图、热键、剪贴板、置顶和 OCR 受限时有可扫描的反馈入口。

## 非目标

- 不导出诊断包、不采集原始日志、不写入持久化诊断记录，不上传任何数据。
- 不显示 `CapabilitySnapshot.notes`、后端原始错误、文件路径、截图像素、OCR 文本、剪贴板内容、窗口标题、显示器 ID 或环境变量。
- 不实现系统设置跳转、Portal、权限绕过、后台探测、自动重试、线程、事件循环或常驻诊断窗口。

## 依赖关系

- 依赖现有 `CapabilitySnapshot`、`GlobalHotkeyHub` 的实际注册状态、`tesseract_available` 与 `TrayFeedback` 的受控文本。
- 依赖 050/054 的 `window_policy`：诊断窗口作为 `Panel` 创建时先隐藏、请求平台任务栏/Dock 隔离，关闭后回到 tray-only。

## 约束

- 诊断快照必须从平台常量、能力布尔值、实际全局热键注册结果、OCR 可用性和受控 tray 反馈建立；热键不得沿用 bootstrap 的平台猜测。
- 恢复建议只由 `ErrorCode` 的固定映射生成；无错误时不得凭空生成错误详情。
- 所有诊断窗口创建与显示必须通过 `window_policy`，不得创建新的事件循环或以常驻窗口替代托盘。
- 不改变 IPC、设置文件、历史资产、截图/贴图/OCR 业务结果或权限语义。

## 阶段

1. 建立纯诊断视图模型和自绘面板，锁定脱敏、状态覆盖与错误恢复建议契约。
2. 以 `Panel` 适配器封装 winit/softbuffer 生命周期；窗口只在 tray 动作触发时创建，关闭即释放。
3. 为 tray 加入诊断动作，将 DesktopApp 的最新受控反馈接线到诊断面板并覆盖窗口事件分发。
4. 执行定向、workspace、跨 target、严格静态和上下文验证，更新稳定事实与残留风险。

## 检查点

1. 注入含路径、换行或后端细节的 `notes` 时，任何诊断行都不包含这些内容。
2. 已注册热键状态覆盖 `CapabilitySnapshot.global_hotkey_available`；OCR 状态来自实际本机引擎可用性。
3. 失败状态同时显示固定错误码与固定恢复建议；成功/进行中状态不虚构错误码。
4. 从 tray 打开、聚焦、关闭诊断窗口均不创建控制窗口，不进入任务栏/Dock/分页器策略之外的窗口路径。

## 计划级风险

- 当前离线测试只能证明诊断内容来源和窗口创建路径，不能证明每个桌面环境实际隐藏任务栏/Dock 或展示 tray 菜单。
- `CapabilitySnapshot` 仍缺少版本化权限/后端粒度；本任务只能把已知布尔状态安全呈现，不能把目标设计误报为已完成的完整能力探针。

## 变更前记录

```text
目的：为受限能力和最近失败提供本地、脱敏、可关闭的诊断反馈入口。
影响路径：tray 动作、DesktopApp 辅助窗口编排、受控 tray 反馈、诊断视图/窗口适配器、上下文和风险。
兼容性：不改 IPC、设置格式、历史/导出数据、截图/贴图/OCR 结果、租户或权限语义；热键仅读取实际注册状态。
外部副作用：仅在用户点选 tray“诊断”时创建既有策略下的本地 Panel；不联网、不读取或上传用户内容、不申请新权限。
回滚点：移除诊断 tray 入口及两个内部模块，tray、窗口策略和业务主流程恢复原有行为。
验证场景：状态脱敏、实际热键覆盖、OCR/能力状态、错误码与恢复建议、tray action、辅助窗口策略、workspace 质量门禁。
```

## 完成标准

- tray 具有“诊断”入口；打开/关闭诊断窗口始终经过 `AuxiliaryWindowKind::Panel`。
- 诊断内容不可携带原始 `notes` 或用户数据，失败状态只含稳定错误码和固定建议。
- 定向、workspace、严格 Clippy、Windows target、差异与 ctx 验证通过；真实桌面菜单与任务栏隔离保留为未覆盖风险。

## 完成记录

- 2026-08-02：新增 `diagnostics_panel` 纯模型和 XRGB 呈现，状态只由平台常量、`CapabilitySnapshot` 的四个公开布尔字段、`GlobalHotkeyHub` 实际注册结果、`tesseract_available` 和 `TrayFeedback` 有限枚举构成；模型不持有、不读取或渲染 `notes`。
- 2026-08-02：新增 `diagnostics_window`，使用既有 `AuxiliaryWindowKind::Panel` 工厂先隐藏创建、映射时应用任务栏/Dock 策略；关闭、Esc 或开始截图后立即释放窗口，不新增事件循环、线程、网络或持久化。
- 2026-08-02：tray 新增“诊断”入口；DesktopApp 保存最近受控反馈并在诊断窗口打开期间刷新状态，失败时只显示 `ErrorCode` 与固定恢复建议，成功或进行中不显示伪造错误。
- 验证：诊断模型 3 项、tray 相关 14 项、window policy 4 项通过；`cargo fmt --check`、workspace check、严格 Clippy、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（应用 232 项、核心 88 项通过，2 项真实桌面测试跳过）、Windows target、差异检查和 ctx validate 通过。
- 未覆盖：未在 Windows、macOS、X11 或 Wayland 原生桌面验证诊断菜单、窗口可见性、任务栏/Dock/分页器隔离、读屏、焦点或能力状态与系统权限的实际一致性；未实现诊断包、复制诊断或完整权限/后端粒度。
