# 计划 023：OCR 受监督应用服务

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/023_ocr_job_service.md`

## 目标

在应用层建立独立的 `OcrJobService`：它提交 `JobSpec`、向 worker 提供协作式取消令牌、接收不可变 OCR 结果，并仅在 `JobSupervisor` 确认 owner 与 `AssetRef` generation 仍有效时交付结果。

## 非目标

- 不修改 `desktop_shell`、winit 事件循环、Overlay 或贴图窗口。
- 不变更 Tesseract 子进程适配器、OCR 文本格式、剪贴板或 UI 展示。
- 不增加线程池、持久队列、并发配置、缓存或网络能力。

## 约束

- worker 只能收到 `CaptureImage` 副本、`JobCancellation` 和结果发送器，不能持有窗口或应用状态。
- 结果只有应用层轮询并经 `JobSupervisor` 接受后才能暴露；owner 关闭、超时、失败和陈旧资产必须有终态。
- 生产 runner 使用现有可取消 OCR 入口；测试 runner 不调用 Tesseract。
- 服务模块必须保持独立，不反向依赖 `desktop_shell`。

## 依赖关系

- 依赖 019 的 `AssetRef` generation。
- 依赖 021 的 `JobSupervisor` 和取消令牌；必要时补齐失败终态和任务元数据查询。
- 依赖 022 的 `recognize_image_with_cancellation` 子进程边界。

## 阶段

1. 扩展监督器的最小失败/元数据查询能力并补测试。
2. 新建可注入 runner 的 `OcrJobService`，由 worker 回传身份引用与结果。
3. 用 fake runner 覆盖成功、失败、owner 关闭、超时和陈旧 generation，验证不依赖 GUI/Tesseract。

## 检查点

- `OcrJobService` 不引用窗口、winit、剪贴板或 Tesseract 命令行参数。
- worker 结果不含窗口句柄；服务在 owner 缺失或 generation 变化时丢弃结果。
- 失败的 OCR worker 只能将运行任务变为失败，不能覆盖取消、超时或关闭终态。

## 计划级风险

- worker thread 仍由服务异步启动，退出编排尚需桌面集成切片取消所有 owner 并等待结果；本任务只建立服务边界。
- 长期并发/优先级需求尚未实施，不能把单一 worker 发射器误称为完整队列。

## 完成标准

- 应用层有可注入、可测试的 OCR 受监督服务，生产 runner 调用既有可取消 OCR 适配器。
- 成功、失败、owner 关闭、超时与陈旧 generation 均有确定性离线测试。
- fmt、check、严格 Clippy、workspace 测试、差异检查和上下文校验通过。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：监督器增加失败终态、任务元数据查询和全量取消；新增可注入 `OcrRunner`、生产 `LocalOcrRunner` 与 `OcrJobService`。worker 只回传 `JobResultRef` 和不可变 OCR 结果，主线程轮询经 owner/generation/终态门禁后才交付。
- 验证：监督器定向测试 6/6、OCR 服务定向测试 6/6、`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（60 app + 39 core 通过，2 个真实桌面测试忽略）、`git diff --check` 与上下文校验通过。
- 残留风险：旧 `desktop_shell` 尚未调用服务，现有贴图/Overlay 入口仍未迁移；服务 worker 的退出等待需在窗口事件循环切片中编排。
