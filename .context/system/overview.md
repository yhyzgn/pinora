# 系统全景：pinora

## 技术与运行基线

- 语言为 Rust，workspace 使用 Edition 2024；版本为 `0.1.0`，见根 `Cargo.toml`。
- Cargo workspace：根 package `pinora` + 成员 `crates/pinora-core`、`crates/pinora-app`。
- 二进制目标名 `pinora`，入口固定为仓库根 `src/main.rs`；`default-run = "pinora"`。
- 第三方依赖：根 package `ctrlc`；`pinora-app` 使用 `fs2`（文件锁）。无真实截图/GUI 依赖。

## 模块边界（已实现）

- 根 `src/main.rs`：唯一进程入口；OS 单实例 bootstrap、fake 捕获演示贴图、轮询 Activate、Ctrl+C 退出。
- `pinora-core`：Command/Event/Error、AppState、几何、`CaptureImage`/`Pin`、`CaptureProvider`/`CaptureRequest`/`DisplayInfo`。
- `pinora-app`：`AppRuntime`（锁 + 探测 + 捕获）、`FakeCaptureProvider`、`OsSingleInstance`（flock + Unix socket）、`InMemorySingleInstance`（测试）。
- 尚未实现：真实 xcap/Portal 截图、GPUI/Liora、托盘、全局热键、Windows/macOS 单实例后端。

## 当前运行行为

- `cargo run`：获取 OS 单实例锁 → Capture 320×180 fake 区域 → CreatePin → 常驻；二次启动转发 Activate 后退出。
- 单实例目录：`$XDG_RUNTIME_DIR/pinora` 或 `/tmp/pinora-$USER`（`instance.lock` + `activate.sock`）。
- Fake 捕获提供虚拟显示器 `fake-0` 1920×1080，非真实屏幕像素。

## 构建、测试与运行

- 编译：`cargo check --workspace`
- 测试：`cargo test --workspace`（当前约 35 个单元测试：core 20 + app 15）
- 运行：`cargo run`

## 外部基础设施与未知项

- 当前实现不访问共享数据库或第三方网络服务。
- 未锁定 GPUI/Liora 版本；真实平台捕获与热键待后续任务。
