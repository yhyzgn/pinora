# 计划 007：导出、剪贴板与动作工作流

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/007_export_hotkey_actions.md`

## 目标

打通「捕获 → 贴图 → 导出/复制」纯逻辑闭环：PNG 落盘、内存剪贴板抽象、`CaptureAndPin`/`SavePng`/`CopyImage` 命令，以及可注入的热键/动作枚举（无真实全局热键后端）。

## 非目标

- 系统剪贴板、真实全局热键、GUI。
- 引入 GPUI/Liora。

## 约束

- 导出实现放在 `pinora-app`；core 只定义命令与端口形状。
- 事件不记录像素；路径仅记录文件名或相对提示时注意脱敏（测试路径可用临时目录）。

## 依赖关系

- 依赖计划 006。

## 阶段

1. AppState 记录 last_capture；扩展 Command/Event。
2. ImageSink（PNG + 内存剪贴板）与 Runtime 接入。
3. ActionId / FakeHotkey 与启动演示导出。
4. 测试与推送。

## 退出标准

- 单元测试覆盖 CaptureAndPin、SavePng、CopyImage。
- `cargo run` 写出 demo PNG 并报告剪贴板登记。

## 检查点

- PNG 文件头合法；剪贴板仅内存实现并有日志说明。
- `ctx validate` 通过。

## 计划级风险

- 路径写入用户 runtime 目录可能残留文件：演示与测试后可手动清理；不写入密钥。

## 完成标准

- 核心闭环在无 GUI 下可测、可观察。
