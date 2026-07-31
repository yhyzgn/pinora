# 任务 015：选区 Overlay 工具栏

- 状态：已完成
- 计划：`.context/plans/015_overlay_toolbar.md`
- 规模：大
- 依赖：`.context/tasks/014_ocr_pin.md`
- 生产行为变更：有

## 任务目标

选区松手后在 Overlay 显示工具栏；双击复制、中键贴图、Enter 贴图；标注/OCR 在 Overlay 完成，不再强制 Enter→独立标注窗。

## 验收标准

- 松手有效选区后可见工具栏按钮区
- 双击复制、中键贴图、Enter 贴图可用
- 工具栏「复制/贴图/保存/OCR」可用
- 选区内可画矩形等基础标注
- `cargo test --workspace` 通过

## 完成记录

- 2026-07-31
- Overlay `Selecting|Ready` + 浮动工具栏（复制/贴图/保存/OCR/标注工具）
- 双击复制、中键/Enter 贴图；选区内标注预览烧录
- 移除独立标注窗主路径
- 验证：`cargo test -p pinora-app --lib` 通过
