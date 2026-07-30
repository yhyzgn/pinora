# 任务 009：区域选区 Overlay 与裁剪工作流

- 状态：已完成
- 计划：`.context/plans/009_region_selection.md`
- 规模：大
- 依赖：`.context/tasks/008_xcap_capture_provider.md`
- 生产行为变更：有

## 任务目标

实现可交互区域选区 Overlay（拖拽 + Esc/Enter），选区确认后裁剪全屏捕获并创建贴图；纯逻辑与集成路径可验证。

## 范围

- core：`SelectionSession`、矩形归一化、`CaptureImage::crop`
- app：`region_overlay`（winit+softbuffer）、交互捕获工作流
- main：启动或动作触发 Overlay 而非固定 320×180

## 非目标

- 跨显示器单一虚拟桌面选区、手柄缩放、文字尺寸 HUD（可用控制台补充）。

## 预期文件

- `crates/pinora-core/src/selection.rs`、`image.rs`
- `crates/pinora-app/src/region_overlay.rs`、工作流模块
- `src/main.rs`、`.context/*`

## 验收标准

- 拖拽产生合法 rect；小于 2×2 不可确认。
- Esc → `None`，不 CreatePin。
- 确认 → 贴图与导出尺寸匹配选区。
- 单元测试不打开真实窗口（Overlay 可 `#[ignore]` 或逻辑分离）。

## 验证

- `cargo test --workspace`
- 手动 `cargo run` 拖拽选区

## 风险与回滚

- 风险：winit 在无显示环境失败。缓解：无 DISPLAY 时回退固定区域。
- 回滚：Action 恢复固定 default_capture_rect。

## 完成记录

- 状态：已完成（2026-07-30）。
- 实际变更：SelectionSession/crop_local；winit+softbuffer Overlay；capture_region_interactive；main 启动走选区。
- 实际验证：`cargo test --workspace` 通过；`cargo run` 进入「preparing region overlay on DP-1」。
- 未解决项：跨屏 Overlay、手柄缩放、尺寸文字 HUD。
