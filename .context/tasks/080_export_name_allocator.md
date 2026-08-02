# 任务 080：桌面导出文件名分配

- 状态：已完成
- 计划：`.context/plans/080_export_name_allocator.md`
- 规模：中
- 依赖：`.context/tasks/026_export_clipboard_job_service.md`、`.context/tasks/027_desktop_export_job_integration.md`、`.context/tasks/028_atomic_png_export.md`、`.context/tasks/037_history_export_integration.md`
- 生产行为变更：是；桌面异步 PNG 保存使用可读且递增的受控文件名。

## 任务目标

替换桌面壳所有 PNG 提交点的内部 `ImageId` 文件名。新的分配器使用 UTC 时间与有限序号，在同秒和已有候选文件时递增；路径仍只能位于既有导出目录。

## 范围

- 新增纯 `export_name` 模块和单元测试。
- 在 `DesktopApp` 初始化并使用路径分配器，覆盖 Overlay 保存、贴图编辑保存、贴图自动保存和贴图菜单保存。
- 更新工作指针、稳定事实和风险。

## 非目标

- 不修改 ExportJob 输入/worker、设置 schema、导出格式/目录、历史协议、原子写入、跨进程排他性或 UI。
- 不新建窗口、事件循环、worker、外部进程、通知或网络请求。

## 预期文件

- `crates/pinora-app/src/{desktop_shell.rs,export_name.rs,lib.rs}`
- `AGENTS.md`
- `.context/plans/080_export_name_allocator.md`
- `.context/tasks/080_export_name_allocator.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 同一秒内连续请求与磁盘上的同名候选生成不同的有限 ASCII PNG 名称，不含内部 ID、用户内容或路径片段。
2. 耗尽候选时返回受控错误，且不提交导出 job、不影响历史索引或现有文件。
3. 所有桌面 PNG 保存入口统一使用分配器，导出、tray 反馈和窗口策略回归通过。

## 验证

- `cargo test -p pinora-app export_name -- --nocapture`
- `cargo test -p pinora-app export_job -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：进程崩溃或第三方在候选检查后创建同名文件。缓解：单实例限制通常并发；既有原子写入保持，跨进程无覆盖保证不在本任务声明。
- 风险：UTC 名称不等同于用户本地时区。缓解：文件名稳定可排序；时区自定义模板为后续独立设置任务。
- 回滚：恢复 `ImageId.png` 的路径计算，保留 worker、历史、数据和窗口策略。

## 完成记录

- 新增纯 `export_name` UTC 日历/序号分配器，覆盖固定格式、闰日、同秒递增、磁盘冲突和检查失败不推进状态。
- `desktop_shell` 的四个异步 PNG 保存入口均已改经该分配器；实际编码、原子写入、历史记录和 tray 导出反馈仍由既有服务处理。
- 2026-08-02 验证通过：定向命名/导出/桌面状态机/窗口策略测试、格式、workspace 编译、严格 Clippy、离线全量测试（app 207 通过、2 项真实桌面测试忽略；core 85 通过）和差异检查。`ctx validate` 待计划风险章节补齐后复核。
- 剩余风险：同目录第三方在候选检查后创建文件的跨进程竞态仍存在；不把单实例的普通冲突规避表述为无覆盖保证。
