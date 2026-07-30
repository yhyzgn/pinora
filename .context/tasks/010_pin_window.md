# 任务 010：winit 贴图窗口

- 状态：已完成
- 计划：`.context/plans/010_pin_window.md`
- 规模：大
- 依赖：`.context/tasks/009_region_overlay.md`
- 生产行为变更：有

## 任务目标

实现贴图桌面窗口：显示截图、拖动、滚轮缩放、置顶、Esc 关闭；多贴图；与 runtime pin 状态联动。

## 范围

- `pinora-app/src/pin_window.rs`、shell 循环
- main 接入
- 可选 core 纯函数：缩放后窗口尺寸

## 非目标

- 全局热键、标注、系统剪贴板。

## 预期文件

- `crates/pinora-app/src/pin_window.rs`
- `crates/pinora-app/src/lib.rs`、`src/main.rs`
- `.context/*`

## 验收标准

- 选区确认后贴图窗口可见。
- 拖动改变位置；滚轮改变缩放；Esc 关闭对应贴图。
- `cargo test --workspace` 通过。

## 验证

- `cargo test --workspace`
- 手动 `cargo run` 选区→贴图

## 风险与回滚

- 风险：与 Overlay 连续两个 EventLoop 的平台差异。缓解：顺序创建/销毁。
- 回滚：去掉 pin_window，恢复仅状态贴图。

## 完成记录

- 状态：已完成（2026-07-30）。
- 实际变更：`pin_window` 多贴图会话；main 选区后展示贴图；ClosePin 同步；Ctrl+N/F2/Ctrl+Q。
- 实际验证：`cargo test --workspace` 通过。
- 未解决项：锁定、透明度滑块、全局热键唤起再截。
