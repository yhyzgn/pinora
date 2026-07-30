# 计划 005：贴图命令接入运行时

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/005_pin_command_dispatch.md`

## 目标

将 `CreatePin` / `ClosePin` / `SetPinTransform` 接入 `AppRuntime` 分发路径，产生对应领域事件，并在启动时创建可观察的演示贴图，使常驻进程具备可验证的贴图状态。

## 非目标

- 不实现真实截图、窗口、GUI。
- 不实现 OS 单实例 IPC。
- 不引入标注/OCR。

## 约束

- 命令仅在 `Running` 阶段成功。
- 事件不携带像素字节。
- 保持 `src/main.rs` 为唯一入口。

## 依赖关系

- 依赖计划 003/004（运行时与领域模型）。

## 阶段

1. 扩展 Command / DomainEvent 与 AppState 图像索引。
2. Runtime 分发与单元测试。
3. 启动演示贴图与上下文同步。

## 退出标准

- `cargo test --workspace` 覆盖贴图创建/关闭/锁定拒绝。
- `cargo run` 启动后 pin 数量 ≥ 1，Ctrl+C 正常退出。

## 检查点

- core 仍无 UI 依赖。

## 计划级风险

- Command 携带 `CaptureImage` 使枚举变大：可接受于进程内；跨进程时再改为 ID 引用。

## 完成标准

- 运行时具备贴图命令闭环的纯逻辑实现。
