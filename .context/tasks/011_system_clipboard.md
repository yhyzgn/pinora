# 任务 011：系统剪贴板图像复制

- 状态：已完成
- 计划：`.context/plans/011_system_clipboard.md`
- 规模：中
- 依赖：`.context/tasks/010_pin_window.md`
- 生产行为变更：有

## 任务目标

确认选区后，截图 PNG 进入系统剪贴板（Linux wl-copy/xclip），其他应用可直接粘贴。

## 范围

- `image_sink.rs`：系统剪贴板写入
- `platform` / overview 能力说明
- 计划与任务文档

## 非目标

- 非 Linux 平台原生 API
- 文本剪贴板

## 预期文件

- `crates/pinora-app/src/image_sink.rs`
- `.context/plans/011_system_clipboard.md`
- `.context/tasks/011_system_clipboard.md`
- `.context/system/overview.md`
- `AGENTS.md` 工作指针

## 验收标准

- `copy_image` 在可用工具下写入 `image/png` 到系统剪贴板
- 工具缺失时返回明确错误或仅内存成功（策略：尽力写入系统，失败记日志仍保留内存）
- 单测：PNG 编码与内存剪贴板；系统路径可 ignore

## 验证

- `cargo test --workspace`
- 可选：`cargo run` 截图后在 Kate/浏览器粘贴

## 风险与回滚

- 风险：无 `wl-copy` 的环境；缓解：检测 PATH 并降级
- 回滚：去掉系统调用，仅内存

## 完成记录

- 状态：已完成（2026-07-31）。
- 实际变更：`LocalImageSink::copy_image` 编码 PNG 后经 `wl-copy`/`xclip`/`xsel` 写入系统剪贴板；失败仅日志，内存副本仍成功。
- 实际验证：`cargo test --workspace` 通过。
- 未解决项：macOS/Windows 原生剪贴板；粘贴到部分应用需手动确认。
