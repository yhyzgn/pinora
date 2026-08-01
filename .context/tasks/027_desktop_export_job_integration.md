# 任务 027：接入桌面导出与剪贴板任务监督

- 状态：已完成
- 计划：`.context/plans/027_desktop_export_job_integration.md`
- 规模：大
- 依赖：`.context/tasks/024_desktop_ocr_integration.md`、`.context/tasks/026_export_clipboard_job_service.md`
- 生产行为变更：是；桌面复制、保存和 OCR 文本复制由同步调用改为后台受监督任务，任务失败将异步记录诊断而不阻塞窗口事件循环。

## 目的

修复 `desktop_shell` 的复制/保存/OCR 文本复制仍在事件循环同步执行的问题，将外部文件和系统剪贴板副作用置于已验证的 owner、资产版本和取消协议之下。

## 任务目标

`DesktopApp` 持有 `ExportJobService` 和最小 pending 元数据；Overlay 完成操作及贴图自动复制/保存提交 `ExportJobInput`，OCR 成功提交文本复制。`about_to_wait` 轮询并记录完成/失败/丢弃；关闭贴图、取消/再截 Overlay 和退出取消 owner 或全量任务。

## 影响路径

- `crates/pinora-app/src/desktop_shell.rs` 的状态、事件循环、Overlay/贴图收尾与 OCR 完成处理。
- `crates/pinora-app/src/export_job.rs` 的完成协议包含 owner，便于 UI 清理 pending 状态。
- 当前计划、任务、`.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 兼容性

- 接口：保留用户现有 Overlay 复制/保存、贴图自动保存/复制、OCR 全文复制的意图；不改领域命令字符串或 `AppRuntime` 的同步兼容 API。
- 数据/状态：不修改持久化、`AppState`、租户或权限语义；新增进程内 job ID 到 owner/资产/动作的 pending 映射。
- 生命周期：任务 worker 仍复用 025/026 的本地适配器；窗口关闭仅取消自身 owner，不影响其他贴图。

## 外部副作用

离线测试只使用 fake runner 与本地文件；不连接网络、共享基础设施或未经授权的真实桌面会话。真实剪贴板测试继续 ignored。

## 回滚点

回滚 `desktop_shell` 的服务字段、pending 映射和提交入口即可恢复旧同步 UI 行为；保留 025/026 的适配器和服务契约。

## 验证场景

- Overlay 复制/保存和贴图自动保存/复制只提交任务，事件循环继续处理热键与窗口事件。
- OCR 成功只提交文本复制任务，不在 OCR 轮询中直接写系统剪贴板；日志不打印 OCR 正文。
- 贴图关闭、Overlay 取消或再截后，晚到导出结果被取消或丢弃，并清理 pending 元数据。
- Overlay 已确认复制/保存并关闭窗口后，其任务仍有冻结资产可完成；无效 generation 不得被接受。

## 范围

- DesktopApp 增加导出服务、pending 元数据和结果轮询。
- 所有桌面复制/保存/OCR 文本复制入口提交 `ExportJobService`。
- 关闭/退出路径取消导出 owner，移除敏感 OCR 预览日志。

## 非目标

- 不改同步 `AppRuntime` 命令、领域事件或现有单元测试的 `LocalImageSink` 兼容行为。
- 不做文件原子替换、用户进度、命名模板、系统剪贴板失败的领域事件重构。
- 不宣称真实 GUI E2E、真实系统剪贴板或子孙进程组回收已通过。

## 预期文件

- `crates/pinora-app/src/desktop_shell.rs`。
- 必要时 `crates/pinora-app/src/export_job.rs` 和相关离线测试。
- `.context/plans/027_desktop_export_job_integration.md`。
- `.context/tasks/027_desktop_export_job_integration.md`。
- `.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- `desktop_shell.rs` 的 UI 复制/保存/OCR 文本路径不再直接调用系统剪贴板或同步导出 action。
- pending 映射只保存 job owner、资产引用和不敏感动作元数据；结果处理遵守终态/generation 门禁并清理映射。
- 关闭/取消/再截/退出都会取消导出任务；严格质量门禁通过，GUI E2E 缺口明确记录。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app export_job::tests -- --nocapture`
- `cargo test -p pinora-app ocr_job::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `rg -n 'copy_text_to_system_clipboard|SaveLastCapture|CopyLastCapture' crates/pinora-app/src/desktop_shell.rs`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：Overlay 关闭后没有活动窗口可提供资产，可能导致已确认的任务被误丢弃。缓解：pending 映射冻结 `AssetRef`，仅任务完成/失败/丢弃后删除；取消/再截显式关闭 owner。
- 风险：服务任务与贴图关闭竞态。缓解：关闭 owner 优先设置不可逆终态，worker 晚到结果只产生丢弃诊断。
- 回滚：恢复 `desktop_shell` 的同步 `ImageSink` 调用和移除 pending 状态；不删除 025/026 契约。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：Overlay Copy/Save 与贴图自动保存/复制通过 `AppRuntime::dispatch` 同步调用 `ImageSink`；OCR 成功轮询直接调用 `copy_text_to_system_clipboard` 并打印最多 240 个字符的 OCR 正文；关闭路径只取消 OCR owner。
- 实际变更：桌面壳新增导出服务、job ID pending 映射与轮询；Overlay Copy/Save、贴图自动保存/复制、OCR 文本复制均提交 `ExportJobInput`。结果按 owner、job ID、当前或冻结资产、截止时间和终态门禁后才记录；贴图关闭、Overlay 取消/再截和退出取消导出任务，日志不再输出 OCR 正文。
- 验证：`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`（3/3）、导出服务 6/6、OCR 服务 6/6；workspace check、严格 Clippy、workspace 测试（69 app 通过/2 忽略，39 core 通过）、静态扫描、差异检查和上下文校验通过。
- 未覆盖项：无真实窗口 E2E；`AppRuntime` 同步 API 仍保留；文件原子导出、worker join/退出收敛和复杂子孙进程回收仍待后续任务。
