# 系统全景：pinora

## 技术与运行基线

- 语言为 Rust，workspace 使用 Edition 2024；版本为 `0.1.0`，见根 `Cargo.toml`。
- 扫描环境为 `rustc 1.95.0`、`cargo 1.95.0`（历史初始化记录）；以当前 `cargo --version` 为准。
- Cargo workspace：根 package `pinora` + 成员 `crates/pinora-core`、`crates/pinora-app`。
- 二进制目标名 `pinora`，入口固定为仓库根 `src/main.rs`；`default-run = "pinora"`。
- 当前无第三方 crates 依赖（仅 path 工作区内依赖）。

## 模块边界（已实现）

- 根 `src/main.rs`：唯一进程入口，只做 bootstrap/shutdown 编排。
- `pinora-core`：纯领域 `Command`（含 `CreatePin`/`ClosePin`/`SetPinTransform`）、`DomainEvent`、`ErrorCode`、`AppState`（图像索引 + 贴图列表）、几何、`CaptureImage`/`RgbaBuffer`、`Pin`/`PinTransform`；不依赖 UI/平台 SDK。
- `pinora-app`：库 crate — `AppRuntime` 命令分发（生命周期 + 贴图）、`SingleInstance` + 内存实现、`FakeCapabilityProbe`。
- 设计文档中的 GPUI/Liora、真实截图、窗口贴图、OCR、热键、托盘等仍为**目标设计，尚未实现**。

## 当前运行行为

- `cargo run`：bootstrap → 创建 320×180 纯色**演示贴图**（非真实截屏）→ 保持运行；Ctrl+C 后优雅 shutdown。尚无 GUI/托盘窗口。
- 单实例为**进程内内存协议**，非 OS 级文件锁；跨进程二次启动转发尚未实现。

## 构建、测试与运行

- 依赖/元数据：`cargo metadata --no-deps --format-version 1`
- 编译：`cargo check --workspace`
- 测试：`cargo test --workspace`（当前 29 个单元测试：core 18 + app 11）
- 运行：`cargo run`

## 外部基础设施与未知项

- 当前实现不访问外部基础设施。
- 未锁定 GPUI/Liora 版本（D-001）；真实平台单实例锁、Portal、热键后端待后续任务。
