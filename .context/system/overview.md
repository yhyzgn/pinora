# 系统全景：pinora

## 技术与运行基线

- 语言为 Rust，workspace Edition 2024，`0.1.0`。
- 根 `src/main.rs` + `pinora-core` + `pinora-app`。
- 依赖：`ctrlc`、`fs2`、`png`、`xcap`、`winit`、`softbuffer`。
- Linux xcap：`pipewire-devel`、`mesa-libgbm-devel` 等。

## 模块边界（已实现）

- **core**：选区 `SelectionSession`、图像 `crop_local`、Capture/Command/Event/Action。
- **app**：`XcapCaptureProvider` / fake 降级、`region_overlay`（全屏拖拽选区）、`capture_region_interactive` 工作流、导出、OS 单实例。
- **main**：启动后进入区域 Overlay；确认后贴图+PNG+内存剪贴板；Esc 取消。

## 当前运行行为

1. 探测捕获后端（优先 xcap）。
2. 捕获主显示器全屏作为 Overlay 背景。
3. 用户拖拽选区，`Enter`/`Space` 确认，`Esc` 取消；方向键微调，`Shift` 加速。
4. 裁剪选区 → CreatePin → 保存 PNG → 内存剪贴板。
5. 进程常驻直至 Ctrl+C。

## 构建、测试与运行

- `cargo test --workspace`（约 47 通过，1 ignored 真捕获）
- `cargo run`（需图形会话）

## 未实现

- 跨屏联合 Overlay、选区手柄缩放、尺寸 HUD 绘制、GPUI 贴图窗、系统剪贴板、全局热键。
