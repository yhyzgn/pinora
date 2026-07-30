# 计划 010：贴图窗口（winit）

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/010_pin_window.md`

## 目标

选区确认后创建可拖动、可滚轮缩放、置顶的无边框贴图窗口；支持多贴图、Esc 关闭、Ctrl+N 再截、Ctrl+Q 退出。

## 非目标

- GPUI/Liora、标注编辑、点击穿透、系统托盘。
- 四角缩放手柄（滚轮缩放即可）。

## 约束

- 贴图像素来自已裁剪 `CaptureImage`；关闭窗口同步 `AppRuntime` ClosePin。
- 不在 `dispatch` 内嵌套第二事件循环之外的逻辑污染；shell 负责窗口。

## 依赖关系

- 依赖计划 009 选区 Overlay。

## 阶段

1. Pin 视图状态与 softbuffer 绘制。
2. 多窗口拖动/缩放/关闭。
3. 接入 main 与 runtime ClosePin。
4. 测试与文档。

## 退出标准

- `cargo run` 选区后出现贴图窗，可拖动与关闭。
- 单元测试覆盖缩放尺寸计算等纯逻辑。

## 检查点

- Wayland 置顶尽力而为。

## 计划级风险

- 再截图需退出再进事件循环：用返回码驱动 shell 循环。

## 完成标准

- 截图→选区→贴图可见闭环。
