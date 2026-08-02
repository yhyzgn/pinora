# 任务 078：托盘内异步状态反馈

- 状态：已完成
- 计划：`.context/plans/078_tray_feedback.md`
- 规模：中
- 依赖：`.context/tasks/024_desktop_ocr_integration.md`、`.context/tasks/027_desktop_export_job_integration.md`、`.context/tasks/056_delayed_capture.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/066_auxiliary_window_visibility_policy.md`
- 生产行为变更：是；tray 菜单和 tooltip 显示受控的最近异步操作状态。

## 任务目标

为现有 tray-only 桌面壳提供不创建新窗口的最近状态反馈。状态由纯模型生成，菜单禁用项和 tooltip 同步显示；只在既有截图、延时、OCR 与导出路径得到真实启动或匹配完成结果后更新。

## 范围

- 新增受限、脱敏、可测试的 tray 反馈模型。
- 扩展 `tray.rs` 的禁用状态项与 tooltip 更新接口。
- 让 OCR、导出服务将资产已变化或 owner 已关闭的 worker 错误保持为 `Discarded`，不把陈旧失败交给 tray。
- 在 `desktop_shell` 的已确认截图、延时、OCR 和导出主链更新反馈。
- 更新工作指针、项目上下文和风险登记。

## 非目标

- 不创建通知/Toast/诊断窗口，不接入系统通知或遥测。
- 不改变截图/OCR/导出任务协议、错误码、持久化、历史、设置、贴图、窗口创建/可见性策略或公共命令。
- 不重写所有日志分支或把未确认的 worker 结果发布给用户。

## 预期文件

- `crates/pinora-app/src/{desktop_shell.rs,export_job.rs,lib.rs,ocr_job.rs,tray.rs,tray_feedback.rs}`
- `AGENTS.md`
- `.context/plans/078_tray_feedback.md`
- `.context/tasks/078_tray_feedback.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. tray 的菜单状态项和 tooltip 使用相同的有限、脱敏文本，展示最近一次截图、延时、OCR 或导出的进行中、成功、失败/取消结果。
2. 只有已有 owner/generation 校验通过的 OCR/导出完成事件才能更新成功或失败状态；陈旧/无归属结果不改变用户反馈。
3. 动态状态与延时截图菜单禁用/取消行为共存；更新失败不影响桌面壳继续 tray 驻留。
4. 不新增窗口、事件循环、worker、系统通知或外部进程；全部辅助窗口继续隐藏创建、唯一展示，禁止任务栏、Dock 与分页器项。

## 验证

- `cargo test -p pinora-app tray_feedback -- --nocapture`
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

- 风险：平台 tray 后端不刷新 tooltip 或菜单标签。缓解：状态模型和调用独立可测，更新无返回依赖且不干预业务路径；真实桌面验收保留。
- 风险：错误详情泄漏内容或路径。缓解：只映射稳定错误码到预定义文案，长度限制，不传递原始错误字符串。
- 风险：陈旧任务结果覆盖新状态。缓解：只在既有 owner/asset 接受后的完成分支更新，未接受结果不产生反馈。
- 回滚：移除反馈模型、tray 状态项和接入调用；保留既有日志、任务与 tray 行为。

## 完成记录

- 新增 `tray_feedback` 纯模型并接入 `AppTray`，禁用菜单状态项和 tooltip 始终取
  用同一受控文案；失败文案只使用稳定错误码，不包含敏感内容或原始错误。
- 桌面壳已在截图、延时截图、OCR 与导出的受确认分支更新最新状态；陈旧/关闭
  owner 的 OCR、导出结果继续仅记录为 `Discarded`。
- 错误 worker 的代际变化和 owner 缺失分别有 OCR、导出回归测试，确认不会把
  陈旧错误发布为用户反馈。
- 2026-08-02 验证通过：定向 OCR/导出/tray/桌面状态机/窗口策略测试、格式、
  workspace 编译、严格 Clippy、离线全量测试、差异检查和上下文校验。
- 剩余风险：真实 Linux/Windows/macOS tray 菜单与 tooltip 的可见性未在本任务
  中自动化；不将离线测试视为原生桌面验收。
