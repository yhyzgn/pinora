# 计划 009：区域选区 Overlay

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/009_region_overlay.md`

## 目标

实现区域截图选区：先捕获目标显示器全屏作为背景，弹出交互 Overlay 拖拽选区，确认后裁剪并贴图/导出；取消不产生截图。

## 非目标

- 多显示器跨屏联合 Overlay（首版主屏/选定显示器）。
- 四角拖拽缩放手柄、标注、GPUI 贴图窗。
- 真实全局热键。

## 约束

- 选区几何与最小尺寸规则在 core 纯逻辑可测。
- Overlay 为阻塞交互，不放进 `AppRuntime::dispatch` 内嵌事件循环。
- Esc 取消；Enter/松手后二次确认可用 Enter。

## 依赖关系

- 依赖计划 008（xcap/fake 捕获）。

## 阶段

1. core 选区会话与图像裁剪。
2. winit Overlay 与软缓冲绘制。
3. 工作流接入 Action/main。
4. 测试与文档。

## 退出标准

- 纯逻辑测试覆盖选区归一化与最小尺寸。
- `cargo run` 可打开 Overlay；确认后导出选区 PNG；Esc 取消不增加 pin。

## 检查点

- Wayland 下窗口可获焦点与指针。
- 取消路径不破坏已有 pins。

## 计划级风险

- Wayland 全屏/光标抓取差异：失败时回退固定区域并记录。

## 完成标准

- 交互选区成为 CaptureRegionAndPin 默认路径。
