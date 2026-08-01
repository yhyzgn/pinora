# 任务 012：标注 + 托盘 + 贴图锁定/透明度

- 状态：已完成
- 计划：`.context/plans/012_annotate_tray_pin_controls.md`
- 规模：大
- 依赖：`.context/tasks/011_system_clipboard.md`
- 生产行为变更：有

## 任务目标

补齐当期基础标注、系统托盘和贴图锁定/透明度控制。

## 范围

- 基础标注图形与烧录。
- 托盘截图/退出菜单。
- 贴图锁定和视觉透明度近似。

## 非目标

- 文本、马赛克、完整 OCR、完整工具栏和真 alpha 窗口透明。

## 预期文件

- `pinora-core` 标注模型。
- `pinora-app` 桌面壳、托盘与贴图控制。
- 对应 `.context` 记录。

## 验收标准

- 标注可烧录到贴图；托盘可触发截图/退出；锁定与透明度操作有可见结果。

## 验证

- `cargo test --workspace`。
- 有图形会话时手动验证标注、托盘和贴图控制。

## 风险与回滚

- GTK 托盘和手工像素渲染具有平台风险；回滚时恢复前一阶段桌面壳路径，不删除 core 数据模型。

## 完成记录

- 2026-07-31
- `pinora-core`：`annotate` 模块（矩形/箭头/画笔、bake）
- `desktop_shell`：标注编辑窗；贴图 L / [ ]
- `tray`：tray-icon 菜单截图/退出
- 验证：`cargo test --workspace` 通过
