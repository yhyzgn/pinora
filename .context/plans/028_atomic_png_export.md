# 计划 028：原子 PNG 导出

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/028_atomic_png_export.md`

## 目标

将本地 PNG 导出从直接写入目标路径改为同目录临时文件、编码完成后同步、原子替换和目标可读性校验，确保文件任务失败、取消或进程中断时不会把半成品发布为成功导出。

## 非目标

- 不实现 JPEG/WebP、命名模板、冲突策略 UI、文件选择器或导出历史。
- 不改变 `ImageSink` trait、导出领域事件或 `ExportJobService` 协议。
- 不宣称 Windows/macOS 原子替换、网络文件系统语义或断电持久性已经验证。

## 约束

- 临时文件必须与目标位于同一目录，才能使用本地文件系统的 rename 原子边界。
- 成功前必须关闭 writer、同步文件内容并确认目标可读取；失败或未提交的临时文件由 RAII 清理。
- 不使用固定临时文件名，不覆盖其他进程的临时文件，不新增依赖。
- 保持 Linux 当前覆盖语义；无法完成替换时返回错误，不发布 `ImageSaved` 成功事件。

## 依赖关系

- 依赖 026 的 `LocalExportRunner` 复用 `save_png_file`。
- 依赖 027 的桌面保存已经经 `ExportJobService` 提交，错误将异步回报而非阻塞 UI。

## 阶段

1. 建立同目录唯一临时文件与 Drop 清理工具。
2. 重写 `save_png_file`：编码、flush/sync、rename、可读性校验。
3. 用离线测试锁定替换旧文件、未提交清理和 PNG 签名；运行全量门禁。

## 检查点

- 不再对目标 PNG 路径直接 `File::create`。
- 目标文件只在临时文件完整编码和同步后才可见；任何提前返回都会清理本任务临时文件。
- 临时文件不会跨目录移动或使用可预测的单一名称。

## 计划级风险

- `std::fs::rename` 的覆盖行为有平台差异；本仓库当前仅声明 Linux 验证，不据此声称跨平台已经可用。
- 仅 `sync_all` 文件不保证目录元数据断电持久化；此计划保证进程内原子发布与可读性，不扩大为断电事务承诺。

## 完成标准

- PNG 导出采用同目录临时写入、内容同步、原子替换和成功后可读性校验。
- 测试覆盖正常替换、未提交临时文件清理和目标 PNG 签名。
- fmt、check、严格 Clippy、workspace 测试、差异检查和上下文校验通过。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：`save_png_file` 现在创建同目录唯一 `AtomicPngTemp`，将完整 PNG 字节写入临时文件并 `sync_all`，关闭后通过 rename 发布到目标路径，最后打开目标验证可读；未提交临时文件在 Drop 时删除。`LocalImageSink` 与 `LocalExportRunner` 均复用该路径。
- 验证：`image_sink::tests` 7/7（1 个真实桌面测试忽略），其中覆盖 PNG 签名、已有目标替换与未提交临时文件清理；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（app 71 项通过、2 个真实桌面测试忽略；core 39 项通过）、静态扫描、`git diff --check` 与上下文校验通过。
- 残留风险：当前仅验证 Linux 本地文件系统的 `rename` 行为；目录元数据未 fsync，不承诺断电事务；退出仍未等待所有 worker 收敛，GUI E2E 未覆盖。
