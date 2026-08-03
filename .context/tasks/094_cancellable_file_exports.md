# 任务 094：可取消的文件保存

- 状态：已完成
- 计划：`.context/plans/094_cancellable_file_exports.md`
- 规模：中
- 依赖：028 原子图片保存、026/027 导出监督、078 tray 反馈、092 多格式保存。
- 生产行为变更：是；tray 可取消运行中的图片文件保存，取消不影响 clipboard、OCR 或其他后台任务。

## 任务目标

将既有协作式 job 取消能力完整接到文件保存用户路径，并在原子发布边界保持真实、可恢复的结果语义。

## 范围

- 为 `ExportJobService` 增加单 `JobId` 取消入口；为 `SaveImage` 编码/临时发布增加协作检查点和临时文件清理。
- 在 `AppTray` 加入动态启停的“取消文件保存”动作，桌面壳只收集并取消运行中的 `PendingExportAction::SaveImage`。
- 扩展 tray 脱敏反馈，区分保存进行、正在取消和已取消；收敛后再移除 pending。
- 添加取消、隔离、临时文件、菜单与反馈回归；更新工作指针、系统全景和风险。

## 预期文件

- `crates/pinora-app/src/{desktop_shell.rs,export_job.rs,image_sink.rs,tray.rs,tray_feedback.rs}`
- `AGENTS.md`
- `.context/plans/094_cancellable_file_exports.md`
- `.context/tasks/094_cancellable_file_exports.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不提供文件进度百分比、打开文件位置、目录选择、模板、覆盖策略、任意文件删除或新的持久化字段。
- 不取消 CopyImage/CopyText、OCR、历史、截图或 overlay/pin 生命周期外的任务。
- 不改变已发布文件、历史协议、编码格式/质量、应用公共命令或窗口/平台 API。

## 验收标准

1. tray 取消项只在运行 `SaveImage` 时可用；触发后仅向这些 job 发送取消，Clipboard job 和其他 owner 保持运行。
2. 保存任务在原子发布前检查取消并清理自身临时文件；取消不会记录历史或覆盖既有目标。已完成发布的文件不被迟到取消删除。
3. pending 映射只在 worker terminal 后清除，取消反馈为固定脱敏文案；陈旧/重复 action 无 panic、无额外副作用。
4. 未新增窗口、事件循环、外部进程或敏感日志，原有 tray/window policy/workspace 回归不破坏。

## 验证

- `cargo test -p pinora-app export_job image_sink tray tray_feedback -- --nocapture`
- `cargo test -p pinora-app desktop_shell -- --nocapture`
- `cargo test -p pinora-app window_policy -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：大图编码/同步阶段的取消响应延后。缓解：发布前后设置明确协作检查点，真实慢盘延迟不作离线结论。
- 风险：取消与原子 rename 竞态。缓解：发布前可清理临时文件，发布后不删除文件并通过稳定状态避免伪造回滚。
- 风险：tray 动态菜单在原生平台延迟。缓解：只在 GUI 线程启停现有 MenuItem，陈旧 action 无副作用；保留原生验证风险。
- 回滚：移除取消项、服务入口和取消检查，保留原子保存、worker、history、clipboard 和 tray 既有行为。

## 完成记录

- 完成时间：2026-08-03。
- 实现结果：文件保存 worker 在可回滚发布边界检查取消；取消请求仅定位运行中的 `SaveImage`，不会触及图像/文本复制、OCR、截图或其他 owner。取消请求后 pending 仅在 worker terminal 收敛时移除，已发布文件保留且不会伪造回滚。
- 回归覆盖：预取消不创建目标或临时目录；单任务取消得到 `Discarded(Cancelled)`；tray 菜单仅映射本动作；反馈为固定脱敏文案；桌面筛选排除 CopyImage、CopyText 和已取消保存任务。
- 门禁：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 277 通过、2 忽略；core 89 通过）、`cargo check --workspace --target x86_64-pc-windows-msvc`、`git diff --check` 与 `ctx validate` 全部通过。
- 风险：离线门禁不证明真实慢盘响应、发布竞态、跨平台 tray 刷新、GUI 流畅性或任务栏/Dock/分页器行为；详见 `.context/system/risks.md` 的 `R-052`。
