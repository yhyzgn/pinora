# 计划 040：系统剪贴板双结果语义

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/040_clipboard_result_semantics.md`

## 目标

修正同步 `LocalImageSink` 将内存副本成功误报为系统剪贴板成功的问题：内存缓存保留供重试，但只有平台剪贴板实际接受 PNG 后才返回成功；系统写入失败返回稳定 `ClipboardFailed`。

## 非目标

- 不改变桌面 `ExportJobService` 的异步任务边界、owner/generation 门禁或系统命令参数。
- 不实现跨平台原生剪贴板 adapter，不引入新 CLI 依赖，不复制 OCR 文本到内存图像缓存。
- 不把无桌面环境测试伪装为系统剪贴板验收；真实剪贴板探针仍需显式授权。

## 约束

- 内存图像副本在编码失败、后端不存在、命令失败、取消或超时后仍可查询并重试。
- 失败错误不得包含二进制路径、剪贴板内容、像素或 OCR 全文；只返回稳定错误码和脱敏摘要。
- 兼容的 `ImageSink` 调用方必须处理 `ClipboardFailed`，不得在失败后发布 ImageCopied 成功事件。

## 阶段

1. 新增 `ErrorCode::ClipboardFailed` 与稳定字符串契约。
2. 修改 `LocalImageSink::copy_image` 双结果语义和脱敏日志。
3. 更新同步 runtime 契约测试和系统事实/风险记录。

## 依赖关系

- 依赖 011 的 `ImageSink`、025 的受控剪贴板进程和 027 的异步导出接入。

## 检查点

- 系统写入成功：返回 `Ok(())`，内存副本与系统结果一致。
- 系统写入失败：返回 `ClipboardFailed`，内存副本仍存在，后续重试可再次调用。
- PNG 编码失败或锁失败：返回对应稳定错误，不发布成功事件。

## 计划级风险

- 旧同步 `AppRuntime` 测试和调用方可能把复制失败当作非致命；必须逐引用处理，不改变异步桌面导出成功语义。
- 真实系统剪贴板仍受桌面会话、Wayland/X11 和工具版本影响；本任务只证明错误边界，不扩展平台支持声明。

## 完成标准

- `ClipboardFailed` 在 core 有稳定字符串测试；LocalImageSink 离线测试证明失败保留内存副本；workspace 质量门禁通过，真实剪贴板探针缺口明确记录。

## 完成记录

- 状态：已完成（2026-08-02）。
- 实际变更：新增 `ErrorCode::ClipboardFailed`；同步 `LocalImageSink` 先保留内存图像，再将 PNG 写入系统剪贴板，系统失败返回稳定错误而不发布成功，日志不输出命令路径或内容。
- 实际变更：保留 `ImageSink` 公共接口与桌面异步 `ExportJobService`，同步 runtime 调用方可识别失败并继续查询内存副本；私有 writer 注入锁定离线失败语义。
- 验证：核心错误码测试 1/1、应用 image_sink 测试 8/8、runtime 测试 11/11；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（152 通过，2 个真实桌面依赖测试忽略）、`git diff --check` 通过。
- 未覆盖项：未执行真实 Wayland/X11 剪贴板读回、Windows/macOS 原生剪贴板和 GUI 失败提示探针；异步 worker 的平台子孙进程风险仍独立登记。
