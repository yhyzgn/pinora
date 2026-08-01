# 任务 025：收敛系统剪贴板子进程生命周期

- 状态：已完成
- 计划：`.context/plans/025_clipboard_process_lifecycle.md`
- 规模：中
- 依赖：`.context/tasks/022_ocr_process_lifecycle.md`、`.context/tasks/024_desktop_ocr_integration.md`
- 生产行为变更：是；系统剪贴板子进程的等待与超时回收路径改变，用户可见成功/失败语义保持不变。

## 目的

修复 `image_sink.rs::pipe_to_cmd` 使用独立线程等待、超时后执行外部 `kill -9` 的生命周期缺陷，使适配器只回收自己创建的 `Child`，并对写入和等待设置有界收敛。

## 任务目标

保留现有后端探测与参数，新增拥有式子进程执行辅助：写入 stdin 后以 `try_wait` 轮询截止时间；正常退出读取输出并检查状态；写入错误或超时先关闭/杀死拥有的 child，再 `wait` 并等待写入线程结束；所有路径返回稳定错误摘要。

## 影响路径

- `crates/pinora-app/src/image_sink.rs` 的系统剪贴板命令执行和离线测试。
- 当前计划、任务、`.context/system/risks.md` 与 `.context/system/overview.md` 的事实记录。

## 兼容性

- 接口：不改变 `ImageSink`、`copy_text_to_system_clipboard`、`copy_image` 或后端探测函数签名。
- 数据/状态：不改变内存剪贴板内容、领域事件、错误码或持久化数据。
- 外部副作用：测试只启动本地 `/bin/sh` fake 命令，不连接桌面共享服务；忽略的真实剪贴板测试保持忽略。

## 外部副作用

生产运行仍可能启动用户选择的 `wl-copy`/`xclip`/`xsel`；本任务只改变其句柄所有权与回收方式，不扩大命令参数或权限。

## 回滚点

恢复 `pipe_to_cmd` 的旧实现即可回退适配器实现；不回退 022、023 或 024 的 OCR 生命周期契约。

## 验证场景

- fake 命令读取 stdin 并正常退出时返回成功。
- fake 命令以非零状态退出时返回包含状态与受限 stderr 的错误。
- fake 命令持续运行超过截止时间时返回超时错误，直接 child 被杀死并等待回收。
- 写入或等待失败时不遗留当前适配器创建的进程和 writer 线程。

## 范围

- 替换 `pipe_to_cmd` 的后台 wait 线程与外部 `kill -9`。
- 增加测试专用命令路径/超时参数注入，覆盖成功和超时回收。
- 更新上下文风险与完成记录。

## 非目标

- 不实现 ClipboardJobService、ExportJobService 或 UI 非阻塞提交。
- 不改 PNG 编码、命名模板、文件原子替换、OCR 文本复制调用点。
- 不声称已经完成真实 Wayland/X11 桌面剪贴板 E2E 或孙进程组回收。

## 预期文件

- `crates/pinora-app/src/image_sink.rs`。
- `.context/plans/025_clipboard_process_lifecycle.md`。
- `.context/tasks/025_clipboard_process_lifecycle.md`。
- `.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- `image_sink.rs` 不再调用外部 `kill`，不再用独立线程承载 `Child::wait_with_output`。
- 直接创建的 `Child` 在正常、失败、写入错误和超时路径均被回收；超时有确定上限。
- 既有内存剪贴板和公共 API 行为不变；离线 fake 命令测试稳定。
- 所有约定质量门禁通过，真实桌面验证缺口明确记录。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app image_sink::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `rg -n 'Command::new\("kill"\)|wait_with_output|thread::spawn' crates/pinora-app/src/image_sink.rs`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：临时文件创建、重绕或 stderr 读取失败可能掩盖子进程状态。缓解：每个步骤返回稳定错误，临时文件由 RAII 清理，成功/超时 fake 命令覆盖直接 child 的主要生命周期。
- 风险：`Child::kill` 只保证直接子进程，不保证其孙进程。缓解：明确记录平台后端的进程组回收缺口，不使用全局 kill 假装解决。
- 回滚：仅回滚 `image_sink.rs` 的辅助函数和测试；保留任务监督与 OCR 子进程改造。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：`pipe_to_cmd` 创建 `Child` 后将 `wait_with_output` 放入独立线程；3 秒超时通过 `Command::new("kill").args(["-9", pid])` 回收，调用方无法持有或取消该等待。
- 实际变更：`pipe_to_cmd` 改为使用唯一临时 stdin/stderr 文件，`wait_for_owned_child` 直接轮询当前 `Child`；超时只调用该句柄的 `kill`/`wait`，stderr 限制为 8 KiB，临时文件 Drop 时清理。新增 `/bin/sh` fake 命令覆盖正常退出和 30ms 超时回收。
- 验证：`cargo test -p pinora-app image_sink::tests -- --nocapture`（5/5 通过、1 忽略）；workspace check、严格 Clippy、workspace 测试（62 app 通过/2 忽略，39 core 通过）、静态扫描、差异检查和上下文校验通过。
- 未覆盖项：无真实系统剪贴板 E2E；`ImageSink` 仍在调用方同步执行，导出/剪贴板尚未进入统一任务监督和 owner/generation 结果协议；孙进程组回收未验证。
