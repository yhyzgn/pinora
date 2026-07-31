# 计划 011：系统剪贴板图像复制

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

## 验收

- 有 `wl-copy` 时，`copy_image` 调用后可用 Ctrl+V 粘贴图像（手动）
- 无工具时不崩溃，仍保留内存剪贴板
- `cargo test --workspace` 通过
