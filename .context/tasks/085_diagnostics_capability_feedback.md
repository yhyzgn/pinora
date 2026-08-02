# 任务 085：诊断与受限能力反馈

- 状态：已完成
- 计划：`.context/plans/085_diagnostics_capability_feedback.md`
- 规模：中
- 依赖：050/054 的 `window_policy`、081/084 的 `GlobalHotkeyHub`、现有 `TrayFeedback`。
- 生产行为变更：用户可从 tray 打开本地诊断面板，查看受控能力状态与最近失败的错误码/恢复建议。

## 任务目标

实现一个可测试的、脱敏的诊断视图模型及其短生命周期窗口适配器，并把它接入 tray 与桌面壳，保持 Pinora 空闲时仅 tray、热键、IPC 与帧缓存存活。

## 范围

- 增加 `diagnostics_panel`：平台常量、能力布尔状态、实际热键结果、OCR 状态、受控 tray 反馈、稳定错误码和恢复建议。
- 增加 `diagnostics_window`：只负责 `Panel` 窗口、softbuffer、刷新、resize、绘制与关闭。
- 为 `TrayAction` 增加诊断入口，并由 `DesktopApp` 创建、聚焦、关闭及刷新该窗口。
- 更新 `AGENTS.md`、085 计划/任务、`.context/system/{overview.md,risks.md}`。

## 预期文件

- `crates/pinora-app/src/{diagnostics_panel.rs,diagnostics_window.rs,desktop_shell.rs,lib.rs,tray.rs,tray_feedback.rs}`
- `AGENTS.md`
- `.context/plans/085_diagnostics_capability_feedback.md`
- `.context/tasks/085_diagnostics_capability_feedback.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不导出诊断包、复制诊断、持久化诊断日志、显示原始后端信息、自动授权、后台探测或加入新的平台依赖。
- 不改变 `CapabilitySnapshot` 公共数据形状，不在业务逻辑中读取环境变量分支，不改变 tray-only、截图、贴图、设置、历史、OCR、导出或 IPC 行为。

## 验收标准

1. 注入敏感 `notes` 时，面板的全部可见行均不含路径、换行、OCR/剪贴板内容或原始后端细节；能力项只由受控字段形成。
2. 诊断中的全局热键状态采用 `GlobalHotkeyHub` 的实际注册结果，OCR 状态采用本机引擎可用性；最近 tray 反馈只使用现有固定文本。
3. 受控失败状态显示稳定 `ErrorCode` 和固定恢复建议；非失败状态不显示伪造错误。
4. tray 动作能被映射为诊断入口；诊断窗口始终使用 `AuxiliaryWindowKind::Panel`，关闭后不保留辅助窗口或新事件循环。

## 验证

- `cargo test -p pinora-app diagnostics_panel -- --nocapture`
- `cargo test -p pinora-app tray -- --nocapture`
- `cargo test -p pinora-app window_policy -- --nocapture`
- `cargo test -p pinora-app desktop_shell -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：错误消息或 `notes` 误入诊断 UI 造成隐私泄漏。缓解：模型 API 不接收 notes，全部文本由有限枚举和常量映射，测试注入敏感内容。
- 风险：诊断窗口绕开窗口策略并进入任务栏。缓解：只使用现有 `Panel` 工厂和静态策略守卫；真实桌面仍需探针。
- 风险：最近状态失真。缓解：DesktopApp 保存最新受控 `TrayFeedback` 并在更新时刷新已打开的窗口；不记录原始错误。
- 回滚：删除诊断入口和内部适配器；现有 tray、能力摘要、业务动作和数据文件保持不变。

## 完成记录

- 完成时间：2026-08-02。
- 交付：`diagnostics_panel` 使用受限字段建立平台、截图、实际全局热键、系统图像剪贴板、置顶和本地 OCR 状态；`notes` 未进入模型。最近状态由 `TrayFeedback` 的固定短标签表示，失败时才显示稳定错误码和固定恢复建议。
- 交付：`diagnostics_window` 只封装 `Panel` 窗口、softbuffer、resize、paint、焦点和关闭；tray 的“诊断”动作可创建或聚焦窗口，关闭/Esc/发起截图后回到 tray-only。打开期间新的 tray 反馈会刷新面板。
- 验证：`cargo test -p pinora-app diagnostics_panel -- --nocapture`（3 项）、`tray`（14 项）、`window_policy`（4 项）、`desktop_shell` 包含于 workspace 测试；`cargo fmt --check`、workspace check、严格 Clippy、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（应用 232 项、核心 88 项，2 项真实桌面测试跳过）、Windows target、`git diff --check`、ctx validate 均通过。
- 已知风险：离线模型与静态窗口策略不证明原生 tray、任务栏/Dock/分页器、焦点、读屏或权限状态；诊断包、复制详情、版本化权限/后端信息继续由 R-046 跟踪。
