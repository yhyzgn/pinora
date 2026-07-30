# 系统全景：pinora

## 技术与运行基线

- 语言为 Rust，crate 使用 Edition 2024；版本为 `0.1.0`，见 `Cargo.toml`。
- 扫描环境为 `rustc 1.95.0 (59807616e 2026-04-14)`、`cargo 1.95.0 (f2d3ce0bd 2026-03-21)`，由版本命令输出确认。
- 当前 crate 是无依赖的单二进制目标 `pinora`；`cargo metadata --no-deps --format-version 1` 显示唯一目标入口为 `src/main.rs`。

## 当前实现边界

- `src/main.rs` 目前只执行 `println!("Hello, world!")`，尚未实现截图、贴图、标注、OCR、热键、托盘或配置功能。
- `docs/Pinora-开发设计文档.md` 是产品与目标架构设计基线（仍需按阶段评审）；其中 GPUI、Liora、xcap、ashpd 等均为设计选型建议，不代表已安装依赖或已实现能力。
- 目录结构目前只有 `src/`、`docs/` 和 Cargo 工程文件；没有数据库、缓存、消息队列或第三方服务配置。

## 构建、测试与运行

- 依赖解析与工程元数据：`cargo metadata --no-deps --format-version 1`，已成功。
- 编译检查：`cargo check`，已成功。
- 测试：`cargo test`，已成功但报告 `0 tests`；目前没有业务测试覆盖。
- 本地运行入口为 `cargo run`，预期输出来自 `src/main.rs` 的 `Hello, world!`；尚未执行长驻桌面运行探针。

## 外部基础设施与未知项

- 当前实现不访问外部基础设施；后续引入截图权限、OCR 模型、剪贴板或平台 Portal 时，必须在对应任务中明确授权和隔离验证方式。
- 尚未确认 GPUI/Liora 的具体版本、平台支持矩阵、发布打包流程和性能基线；这些属于待评审设计，不应在代码变更前视为既定事实。
