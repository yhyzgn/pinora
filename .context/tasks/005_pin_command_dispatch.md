# 任务 005：贴图命令分发与演示贴图

- 状态：已完成
- 计划：`.context/plans/005_pin_commands.md`
- 规模：中
- 依赖：`.context/tasks/004_core_domain_models.md`
- 生产行为变更：有（启动后创建演示贴图；命令面扩展）

## 任务目标

扩展命令/事件，在 `AppRuntime` 中处理贴图创建/关闭/变换，启动时放入一张纯色演示截图贴图。

## 范围

- `Command`：`CreatePin`、`ClosePin`、`SetPinTransform`
- `DomainEventKind`：`PinCreated`、`PinClosed`、`PinUpdated`
- `AppState`：保留图像索引
- `AppRuntime::dispatch` 分支与测试
- `src/main.rs` 演示路径

## 非目标

- GUI 贴图窗口、真实屏幕捕获。

## 预期文件

- `crates/pinora-core/src/{command,event,state}.rs`
- `crates/pinora-app/src/runtime.rs`
- `src/main.rs`
- `.context/*`、`AGENTS.md`

## 验收标准

- Running 下 CreatePin 增加 pin 并发出 PinCreated。
- ClosePin 移除 pin；不存在时 NotFound。
- 非 Running 时贴图命令失败。
- 启动日志显示 demo pin 与 pin 数量。

## 验证

- `cargo test --workspace`
- `cargo run` + SIGINT 退出探针

## 风险与回滚

- 风险：演示贴图被误认为真实截图。缓解：日志标明 demo solid buffer。
- 回滚：移除新命令分支与 main 演示代码。

## 完成记录

- 状态：已完成（2026-07-30）。
- 实际变更：Command/Event 扩展贴图三类操作；AppState 保留 images；Runtime 分发；main 启动创建演示贴图。
- 实际验证：`cargo test --workspace` 29 passed；`cargo run` 输出 demo pin pins=1 且 Ctrl+C 正常退出。
- 未解决项：真实截图、GUI 贴图窗口、OS 单实例。
