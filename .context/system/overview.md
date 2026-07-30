# 系统全景：pinora

## 技术与运行基线

- 语言为 Rust，workspace 使用 Edition 2024；版本为 `0.1.0`。
- 根 package `pinora`（`src/main.rs`）+ `crates/pinora-core` + `crates/pinora-app`。
- 第三方依赖：根 `ctrlc`；app 侧 `fs2`（文件锁）、`png`（导出编码）。无 GUI/真截屏依赖。

## 模块边界（已实现）

- **core**：Command/Event/ActionId、CaptureProvider、ImageSink trait、几何/CaptureImage/Pin、AppState（含 last_capture/last_pin）。
- **app**：AppRuntime（锁+探测+捕获+导出）、FakeCaptureProvider、LocalImageSink、FakeHotkeySource、OsSingleInstance。
- **main**：OS 单实例 → InvokeAction 捕获贴图 → SavePng → CopyImage → 常驻轮询 Activate/假热键。

## 当前运行行为

- `cargo run` 输出捕获尺寸、pin、导出路径（runtime 目录 `export/*.png`）、内存剪贴板字节数；Ctrl+C 退出。
- 二次 `cargo run` 转发 Activate。
- 热键仅为 Fake 注册表（可 inject），**非**系统全局热键。
- 捕获为 fake 纯色，**非**真实屏幕。

## 构建、测试与运行

- `cargo check --workspace`
- `cargo test --workspace`（约 39 个单元测试：core 21 + app 18）
- `cargo run`

## 未实现

- GPUI/Liora、真截屏、系统剪贴板、全局热键、托盘、标注/OCR。
