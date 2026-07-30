# 任务 006：CaptureProvider + OS 单实例

- 状态：已完成
- 计划：`.context/plans/006_capture_and_os_instance.md`
- 规模：大
- 依赖：`.context/tasks/005_pin_command_dispatch.md`
- 生产行为变更：有

## 任务目标

实现可注入的截图能力抽象（含 fake）与 Unix OS 单实例锁/激活转发，并接入 `AppRuntime` 与 `src/main.rs`。

## 范围

- core：`DisplayInfo`、`CaptureRequest`、`CaptureProvider`、相关 Command/Event。  
- app：`FakeCaptureProvider`、`OsSingleInstance`、Runtime 第三泛型参数。  
- main：使用 OS 单实例 + fake 捕获创建演示贴图；循环 `poll_forwarded`。  
- 能力探测标记 fake 捕获可用。

## 非目标

- 真实屏幕像素捕获、非 Unix 单实例。

## 预期文件

- `crates/pinora-core/src/capture.rs` 及 command/event/lib  
- `crates/pinora-app/src/{capture,os_instance,runtime,single_instance,platform,lib}.rs`  
- `src/main.rs`、`Cargo.toml`、`.context/*`

## 验收标准

- `Capture` 命令写入 `images` 并产生 `CaptureCompleted`。  
- `CreatePinFromImage` 可引用已捕获图像。  
- 同路径下第二把 OS 锁返回 ExistingInstance；forward Activate 后主实例 `poll_forwarded` 增加 activation_count。  
- `cargo test --workspace` 通过；手动双开行为符合预期。

## 验证

- `cargo test --workspace`  
- 启动两个 `cargo run` 进程的探针（或集成测试）

## 风险与回滚

- 残留 socket/lock：release 与 Drop 清理。  
- 回滚：恢复 InMemory 单实例与纯 solid demo pin。

## 完成记录

- 状态：已完成（2026-07-30）。
- 实际变更：`CaptureProvider`/`CaptureRequest`/`Command::Capture`/`CreatePinFromImage`；`FakeCaptureProvider`；`OsSingleInstance`；Runtime 三依赖注入；main 走 OS 单实例 + capture→pin。
- 实际验证：`cargo test --workspace` 35 passed；双进程探针 secondary 转发 Activate，primary `activation_count=1` 后 Ctrl+C 退出 0。
- 未解决项：真实屏幕捕获、非 Unix 单实例、GUI。
