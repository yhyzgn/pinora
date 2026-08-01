# 计划 026：导出与剪贴板受监督服务

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/026_export_clipboard_job_service.md`

## 目标

在应用层建立统一的 `ExportJobService`，为 PNG 文件保存、图像剪贴板和 OCR 文本剪贴板提供可注入 runner、任务 owner、资产 generation、截止时间和结果门禁。服务先作为独立契约存在，不直接改动遗留 `desktop_shell` 事件流。

## 非目标

- 不在本计划中接入 `desktop_shell`、重写 `AppRuntime` 同步命令或改变现有领域事件。
- 不实现文件命名模板、JPEG/WebP、历史记录、进度 UI、并发队列或重试策略。
- 不把服务单测描述为真实系统剪贴板或 GUI E2E。

## 约束

- worker 只持有 `CaptureImage`/文本副本、路径、取消令牌和结果发送器，不持有窗口、runtime、owner map 或 UI 回调。
- `JobKind::Export` 只接受文件保存；`JobKind::Clipboard` 只接受图像或文本复制。
- 结果只有在 `JobSupervisor` 接受匹配 `AssetRef`、owner 仍有效且任务未终态时才交付；runner 的外部副作用必须在取消前检查令牌。
- 生产 runner 复用 `LocalImageSink` 的 PNG/系统剪贴板适配；测试 runner 不启动外部命令。

## 依赖关系

- 依赖 021 的 `JobSupervisor`、`JobKind::Export/Clipboard` 与 owner/generation 契约。
- 依赖 025 的拥有式剪贴板 child 适配器。
- 后续 027 才将服务接入 `desktop_shell`，本计划不修改窗口生命周期。

## 阶段

1. 抽象导出输入和可注入 `ExportRunner`，定义统一完成/失败/丢弃结果。
2. 实现生产 `LocalExportRunner`，复用既有 PNG 和文本/图像剪贴板入口。
3. 用 fake runner 覆盖三种操作成功、失败、owner 关闭、超时和陈旧 generation。

## 检查点

- worker 不引用 `winit`、窗口句柄、`AppRuntime` 或 `desktop_shell`。
- 服务错误不会覆盖取消、owner 关闭、超时或已完成终态。
- 文本全文和像素只存在于 worker 输入与 runner 调用，不进入 `JobSpec`、日志或完成事件。

## 计划级风险

- 服务完成前，旧 UI 仍可能同步调用 `ImageSink`；这不是本计划的完成条件，必须在 027 明确迁移并补 UI 关闭/退出路径。
- 生产 runner 的系统剪贴板命令虽然已有直接 child 回收，但取消只能在适配器检查点生效；需要后续将取消令牌传入命令轮询。

## 完成标准

- 一个应用服务统一表达保存 PNG、复制图像和复制文本三类任务，并由 `JobSupervisor` 门禁结果。
- fake runner 离线测试覆盖成功、失败、取消/owner 关闭、截止时间与陈旧 generation。
- fmt、check、严格 Clippy、workspace 测试、差异检查和上下文校验通过。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：新增 `ExportJobInput`、可注入 `ExportRunner`、生产 `LocalExportRunner` 与 `ExportJobService`。服务按 `JobKind::Export/Clipboard` 校验输入，worker 只执行本地 runner 并回送 `JobResultRef`；主线程轮询经 owner、generation、截止时间和终态门禁后才交付完成/失败/丢弃结果。剪贴板取消令牌已传入 025 的 child 等待轮询。
- 验证：`export_job::tests` 6/6、`image_sink::tests` 5/5（1 个真实桌面测试忽略）；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（app 68 项通过、2 个真实桌面测试忽略；core 39 项通过）、静态扫描、`git diff --check` 与上下文校验通过。
- 残留风险：服务尚未接入 `desktop_shell` 或 `AppRuntime`，旧同步复制/保存仍是生产 UI 路径；生产 runner 的文件保存尚非原子替换，系统剪贴板孙进程组和真实 GUI E2E 未验证。
