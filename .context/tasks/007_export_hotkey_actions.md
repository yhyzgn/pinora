# 任务 007：PNG 导出、内存剪贴板与 CaptureAndPin

- 状态：已完成
- 计划：`.context/plans/007_export_and_actions.md`
- 规模：中
- 依赖：`.context/tasks/006_capture_os_instance.md`
- 生产行为变更：有

## 任务目标

实现图像导出端口（PNG 保存 + 内存剪贴板）、`CaptureAndPin`/`SavePng`/`CopyImage`/`InvokeAction` 命令，以及 Fake 热键动作源。

## 范围

- core：ActionId、命令/事件扩展、AppState last_capture
- app：LocalImageSink（png crate）、FakeHotkeySource、Runtime 注入 ImageSink
- main：演示 capture+pin+save+copy

## 非目标

- 系统级剪贴板与全局热键注册。

## 预期文件

- `crates/pinora-core/src/{action,export,command,event,state,lib}.rs`
- `crates/pinora-app/src/{image_sink,hotkey,runtime,lib}.rs`
- `src/main.rs`、`.context/*`

## 验收标准

- SavePng 生成可读 PNG 文件头。
- CopyImage 后 sink 可查询到同一 image_id。
- CaptureAndPin 一次命令完成捕获与贴图。
- 测试与 run 探针通过。

## 验证

- `cargo test --workspace`
- `cargo run` 检查 runtime 目录 demo.png

## 风险与回滚

- 风险：误以为系统剪贴板/全局热键已可用。缓解：日志与 overview 标明 memory/fake。
- 回滚：移除 ImageSink 注入与新命令分支。

## 完成记录

- 状态：已完成（2026-07-30）。
- 实际变更：ActionId/InvokeAction、CaptureAndPin、SavePng/CopyImage、LocalImageSink(png)、FakeHotkeySource；main 演示完整导出闭环。
- 实际验证：`cargo test --workspace` 39 passed；`cargo run` 写出 export PNG 并登记内存剪贴板。
- 未解决项：系统剪贴板与全局热键。
