# 任务 015：选区 Overlay 工具栏

- 状态：已完成
- 计划：`.context/plans/015_overlay_toolbar.md`
- 规模：大
- 依赖：`.context/tasks/014_ocr_pin.md`
- 生产行为变更：有

## 任务目标

选区松手后在 Overlay 显示工具栏；双击复制、中键贴图、Enter 贴图；标注/OCR 在 Overlay 完成，不再强制 Enter→独立标注窗。

## 范围

- `Selecting | Ready` Overlay 状态。
- 工具栏布局、命中、复制、贴图、保存、OCR 和基础标注预览。
- 双击、中键、Enter、Esc 与重新框选交互。

## 非目标

- 选区缩放手柄、完整图标资源、多显示器联合 Overlay。
- 彻底删除旧独立标注窗或贴图路径。

## 预期文件

- `crates/pinora-app/src/desktop_shell.rs`。
- `crates/pinora-app/src/overlay_toolbar.rs`。
- 当前计划、任务、系统全景与 `AGENTS.md` 指针。

## 验收标准

- 松手有效选区后可见工具栏按钮区
- 双击复制、中键贴图、Enter 贴图可用
- 工具栏「复制/贴图/保存/OCR」可用
- 选区内可画矩形等基础标注
- `cargo test --workspace` 通过

## 验证

- `cargo test -p pinora-app --lib`。
- 有图形会话时手动验证工具栏命中、双击复制、中键/Enter 贴图和 Esc 取消。

## 风险与回滚

- 统一桌面事件循环和手工帧缓冲会引入性能/命中回归；回滚时恢复前一阶段独立标注窗主路径或移除工具栏入口。
- 多显示器、HiDPI 和系统剪贴板真实行为仍需隔离桌面探针。

## 完成记录

- 2026-07-31
- Overlay `Selecting|Ready` + 浮动工具栏（复制/贴图/保存/OCR/标注工具）
- 双击复制、中键/Enter 贴图；选区内标注预览烧录
- 移除独立标注窗主路径
- 验证：`cargo test -p pinora-app --lib` 通过
