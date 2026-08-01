# 计划 025：系统剪贴板子进程生命周期

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/025_clipboard_process_lifecycle.md`

## 目标

收敛 `LocalImageSink` 调用 `wl-copy`、`xclip` 或 `xsel` 时的子进程边界：子进程句柄由当前适配器持有，写入和等待有明确截止时间，超时通过拥有的 `Child` 协作回收，不再创建脱离生命周期的等待线程或调用外部 `kill` 命令。

## 非目标

- 不在本计划中改变 `ImageSink` trait、`AppRuntime` 命令协议或系统剪贴板成功/失败语义。
- 不在本计划中把剪贴板/导出入口接入 `JobSupervisor`；异步任务服务作为后续独立切片。
- 不增加平台 SDK、第三方依赖、网络访问或真实桌面验证。

## 约束

- 保持现有 Linux 剪贴板后端探测顺序和命令参数。
- 系统剪贴板命令失败仍只影响系统副本，内存图像副本和既有降级行为不变。
- 子进程超时必须经过 `Child::kill` 后 `Child::wait` 完成回收；不得依赖 PID 字符串、外部命令或后台线程等待。
- 写入线程若为避免阻塞而存在，必须绑定当前子进程，并在子进程终止后有界收敛，不得持有窗口或应用状态。

## 依赖关系

- 依赖 022 的外部 OCR 子进程回收原则和既有错误码语义。
- 依赖 024 的桌面 OCR owner 生命周期已闭环；本计划不改 OCR。

## 阶段

1. 将剪贴板命令执行抽成拥有 `Child` 的有界等待辅助函数。
2. 在超时、写入失败、非零退出和正常退出路径统一回收 stdin、stderr 与子进程。
3. 用离线 fake 命令覆盖成功和超时回收，并运行全量质量门禁。

## 检查点

- `image_sink.rs` 不再出现 `Command::new("kill")`、`wait_with_output` 独立线程或未回收的 `Child`。
- 正常退出和超时都能得到稳定错误/成功结果，超时不会让测试或 UI 永久等待。
- 不改变 `LocalImageSink::copy_image` 的内存剪贴板行为和现有公共 API。

## 计划级风险

- 该切片只修复适配器本身的进程生命周期，调用方仍可能在 UI 事件线程同步等待；后续任务必须把复制/导出迁移到受监督 worker。
- 子进程可能再派生孙进程；当前只承诺回收 `LocalImageSink` 直接创建的 `Child`，复杂平台后端需单独探针验证。

## 完成标准

- 系统剪贴板命令由适配器直接持有并回收，超时不使用外部 `kill`。
- 成功、非零退出、写入失败和截止时间路径有离线契约测试或明确静态证据。
- fmt、check、严格 Clippy、workspace 测试、差异检查和上下文校验通过。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：`LocalImageSink` 使用拥有式临时 stdin/stderr 文件，直接持有并轮询 `Child`；正常退出读取有限 stderr，超时先 `Child::kill` 再 `Child::wait`，删除独立 `wait_with_output` 线程和外部 `kill` 命令。临时文件由 RAII 清理，避免管道写入和 stderr 管道填满造成死锁。
- 验证：剪贴板定向测试 5/5 通过、1 个真实桌面测试忽略；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（app 62 项通过、2 个真实桌面测试忽略；core 39 项通过）、静态扫描、`git diff --check` 与上下文校验通过。
- 残留风险：`ImageSink` 调用仍是同步 API，尚未绑定 `JobSupervisor` 或窗口 owner；直接子进程的孙进程组回收和真实 Wayland/X11 剪贴板 E2E 仍待验证。
