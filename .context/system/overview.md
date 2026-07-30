# 系统全景：pinora

## 技术与运行基线

- 语言为 Rust，workspace 使用 Edition 2024；版本为 `0.1.0`。
- 根 package `pinora`（`src/main.rs`）+ `crates/pinora-core` + `crates/pinora-app`。
- 第三方依赖：根 `ctrlc`；app 侧 `fs2`、`png`、`xcap`（真实截屏）。
- Linux 构建/运行 xcap 需要系统库：`pipewire-devel`、`mesa-libgbm-devel`（及常见 wayland/xcb）。

## 模块边界（已实现）

- **core**：Command/Event/ActionId、CaptureProvider、ImageSink、几何/CaptureImage/Pin、AppState。
- **app**：AppRuntime、`XcapCaptureProvider`、`FakeCaptureProvider`、`SelectedCaptureProvider::autodetect`（真捕获优先、失败降级）、LocalImageSink、FakeHotkeySource、OsSingleInstance。
- **main**：探测捕获后端 → OS 单实例 → CaptureAndPin → SavePng → CopyImage → 常驻。

## 当前运行行为

- 在本机 Wayland 会话中验证过：`capture backend = xcap`，主屏例如 `DP-1 3840x2160`，区域 320×180 真像素导出 PNG。
- 若 xcap 枚举或捕获失败，自动降级 `fake` 并在 capability notes 中说明。
- 剪贴板仍为内存；热键仍为 Fake 注册表。

## 构建、测试与运行

- `cargo check --workspace`
- `cargo test --workspace`（约 40 通过；真捕获用例 `#[ignore]`，可 `cargo test -p pinora-app real_capture -- --ignored`）
- `cargo run`

## 未实现

- GPUI/Liora 贴图窗口、Overlay 选区、系统剪贴板、全局热键、标注/OCR。
