# 计划 027：桌面导出与剪贴板任务接入

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/027_desktop_export_job_integration.md`

## 目标

将 `desktop_shell` 的 Overlay 复制/保存、贴图自动保存/复制和 OCR 成功后的文本复制统一提交给 `ExportJobService`，使耗时操作不再阻塞窗口事件循环，并在贴图关闭、Overlay 取消、再截和应用退出时取消对应 owner 的任务。

## 非目标

- 不改 `AppRuntime::dispatch` 的同步 `ImageSink` 兼容命令，不重写领域事件协议。
- 不实现导出进度 UI、文件选择器、命名模板、原子文件替换或队列配置。
- 不进行未授权真实桌面剪贴板/GUI E2E，也不声称服务单测等价于窗口闭环。

## 约束

- UI 事件只构造 `JobSpec` 和不可变输入，worker 不触碰窗口、runtime、贴图 map 或剪贴板 UI。
- Overlay 复制/保存关闭窗口后仍保留对应任务的 `AssetRef`，直至结果处理；取消/再截才关闭 owner。
- 贴图关闭、Overlay 取消、再截和退出同时取消 OCR 与导出 owner；晚到结果只记录丢弃诊断。
- 日志不得写入 OCR 全文、图像像素或剪贴板正文。

## 依赖关系

- 依赖 024 的 OCR 桌面接入和 owner/generation 门禁。
- 依赖 025 的拥有式系统剪贴板 child 回收。
- 依赖 026 的 `ExportJobService`、可取消 runner 与完成协议。

## 阶段

1. 为 DesktopApp 添加导出服务和 pending 任务元数据，在事件循环轮询完成结果。
2. 改写 Overlay 和贴图的复制/保存、OCR 文本复制入口，删除桌面壳对同步 `ImageSink` 调用。
3. 补齐 owner 关闭/退出取消路径，删除 OCR 全文日志并运行静态和离线回归。

## 检查点

- `desktop_shell.rs` 不再直接调用系统文本剪贴板或 `InvokeAction(SaveLastCapture/CopyLastCapture)`。
- 完成、失败和丢弃结果在事件循环中清理 pending 元数据；贴图/Overlay owner 关闭会取消正在运行的任务。
- Overlay 完成后关闭窗口不会错误取消其已确认的复制/保存任务，但用户取消或再截会取消尚未确认的任务。

## 计划级风险

- 当前 `desktop_shell` 为单体，新增 pending 映射会增加局部复杂度；仅保持 job ID 到最小元数据的映射，不把窗口句柄或内容传入。
- 旧 `AppRuntime` 仍保留同步导出以兼容领域测试；后续需要单独设计命令级异步事件协议，不能将其遗漏误称为完全迁移。

## 完成标准

- 桌面 UI 的复制、保存和 OCR 文本复制均经 `ExportJobService` 提交与轮询。
- owner 关闭、取消、再截、超时和 generation 失效都不会让晚到结果更新窗口状态。
- fmt、check、严格 Clippy、workspace 测试、差异检查和上下文校验通过。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：`DesktopApp` 持有 `ExportJobService` 与按 job ID 保存 owner、`AssetRef`、动作元数据的 pending 映射；事件循环轮询完成、失败和丢弃结果。Overlay 复制/保存、贴图自动保存/复制和 OCR 成功文本复制均改为提交后台任务。Overlay 关闭后已确认的复制/保存使用 job ID 对应冻结资产，活跃贴图仍优先校验当前资产；关闭贴图、取消/再截 Overlay 和退出取消对应导出任务。移除 OCR 正文预览日志。
- 验证：桌面 pending 资产测试 3/3、`export_job::tests` 6/6、`ocr_job::tests` 6/6；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（app 69 项通过、2 个真实桌面测试忽略；core 39 项通过）、静态扫描、`git diff --check` 与上下文校验通过。
- 残留风险：`AppRuntime` 仍保留同步 `ImageSink` 兼容命令但不再是桌面 UI 路径；文件保存尚非原子替换；退出只发取消请求而不等待所有 worker 收敛；未进行真实系统剪贴板或 GUI E2E。
