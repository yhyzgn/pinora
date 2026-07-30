# 计划 004：领域核心数据模型

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/004_core_domain_models.md`

## 目标

在 `pinora-core` 实现设计文档中的最小可测领域模型：几何、截图像素元数据、贴图变换与 `AppState` 实体集合，为后续截图/贴图服务提供稳定类型。

## 非目标

- 不引入第三方依赖、不实现 GUI、截图、平台 API。
- 不实现标注引擎完整算法或 OCR。
- 不改变进程入口布局（保持 `src/main.rs`）。

## 约束

- 仅修改 `pinora-core` 及必要的上下文/文档指针；保持 std-only。
- 类型命名与设计文档术语表对齐；像素坐标使用明确类型。

## 依赖关系

- 依赖计划 003（workspace 与 AppRuntime 已完成）。

## 阶段

1. 几何与 ID 类型。
2. CaptureImage / Pin / PinTransform 与 AppState 扩展。
3. 单元测试与上下文同步。

## 退出标准

- `cargo test --workspace` 通过且新增领域模型测试。
- `AppState` 可持有 pins 列表的纯逻辑增删。

## 检查点

- core 无 app/UI 依赖。
- 用户审查前按约定提交（本阶段用户已要求持续开发与推送时再提交）。

## 计划级风险

- 过早固化像素布局字段：用文档注释标明可演进。

## 完成标准

- 领域类型可供后续 capture/pin crate 直接引用。
