# 任务 013：标注工具增强

- 状态：已完成
- 计划：`.context/plans/013_annotate_tools_plus.md`
- 规模：中
- 依赖：`.context/tasks/012_annotate_tray_pin_controls.md`
- 生产行为变更：有

## 任务目标

扩展标注：椭圆、马赛克、文本（系统 CJK 字体）、颜色循环、线宽调节，并接入标注窗快捷键。

## 范围

- `crates/pinora-core/src/annotate.rs`（及 Cargo 依赖 `fontdue`）
- `crates/pinora-app/src/desktop_shell.rs` 标注事件
- `.context/system/overview.md`、工作指针

## 非目标

- OCR、工具条 GUI、设置持久化

## 预期文件

- `crates/pinora-core/src/annotate.rs`
- `crates/pinora-core/Cargo.toml`
- `crates/pinora-app/src/desktop_shell.rs`
- `.context/*`

## 验收标准

- 椭圆/马赛克拖拽可烧录；文本可键入并烧录。
- C 换色、+/- 调线宽。
- `cargo test --workspace` 通过。

## 验证

- `cargo test --workspace`
- 手动标注流程（可选）

## 风险与回滚

- 风险：无系统字体时文本不可见。缓解：多路径探测 + 测试跳过无字体环境。
- 回滚：还原 annotate 与 shell 键位。

## 完成记录

- 2026-07-31
- `pinora-core`：Ellipse/Mosaic/Text + 调色板/线宽；`fontdue` 系统 CJK 字体栅格化
- `desktop_shell`：1–6 工具、C 颜色、+/- 线宽、文本 IME/键入、Enter/Esc 语义
- 验证：`cargo test -p pinora-core -p pinora-app --lib` 通过
