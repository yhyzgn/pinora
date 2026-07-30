# 计划 008：真实屏幕捕获（xcap）与降级

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/008_xcap_capture_provider.md`

## 目标

接入 `xcap` 作为真实 `CaptureProvider`，启动时探测可用性；失败则降级 `FakeCaptureProvider`，保证无权限环境仍可运行与测试。

## 非目标

- Overlay 选区 UI、Portal 专用授权 UI。
- GPUI 贴图窗口。
- Windows/macOS 专项打磨（依赖 xcap 自带实现即可）。

## 约束

- 离线单元测试不得依赖真实显示器；真捕获用 `#[ignore]` 或显式 env 探针。
- 业务层仍只依赖 `CaptureProvider` trait。
- 不把像素写入日志。

## 依赖关系

- 依赖计划 006/007。

## 阶段

1. 添加 xcap，实现 `XcapCaptureProvider`。
2. 启动选择逻辑与能力探测文案。
3. 可选集成探针与上下文同步。

## 退出标准

- `cargo test --workspace` 全部通过（无真实捕获硬依赖）。
- `cargo run` 在可用环境使用 xcap，否则明确降级到 fake。

## 检查点

- 区域坐标相对显示器转换正确。
- Wayland 无权限时错误映射清晰。

## 计划级风险

- Wayland 权限/合成器差异导致捕获失败：必须降级而非崩溃。

## 完成标准

- 真捕获路径可编译、可探测、可降级。
