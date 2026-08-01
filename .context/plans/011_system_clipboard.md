# 计划 011：系统剪贴板图像复制

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/011_system_clipboard.md`

## 目标

Enter 确认截图后，除内存剪贴板外，将 PNG 写入**系统剪贴板**，便于粘贴到浏览器/IM/文档。

## 范围

- Linux：优先 `wl-copy`（Wayland），回退 `xclip`（X11）
- 保持 `LocalImageSink` 内存副本（测试与降级）
- 更新 overview / conventions / 任务

## 非目标

- macOS/Windows 原生剪贴板（后续平台任务）
- 文本剪贴板 / OCR
- 托盘、标注

## 约束

- 系统剪贴板失败不得让内存副本操作崩溃。
- 本阶段只记录 Linux 后端，不宣称 Windows/macOS 已支持。

## 依赖关系

- 依赖 `.context/tasks/010_pin_window.md` 提供截图与贴图基础路径。

## 阶段

1. 保留内存剪贴板作为测试/降级路径。
2. 调用 Linux 剪贴板命令写入 PNG。
3. 记录手动验证和失败降级。

## 检查点

- 缺少 `wl-copy`/`xclip` 时必须有明确反馈。
- 用户审查前不提交、不推送。

## 计划级风险

- 外部剪贴板进程在无图形会话时可能失败或阻塞。

## 完成标准

- Linux 路径、内存降级和 workspace 测试结果均已记录。

## 验收

- 有 `wl-copy` 时，`copy_image` 调用后可用 Ctrl+V 粘贴图像（手动）
- 无工具时不崩溃，仍保留内存剪贴板
- `cargo test --workspace` 通过
