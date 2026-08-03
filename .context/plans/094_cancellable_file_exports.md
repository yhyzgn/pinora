# 计划 094：可取消的文件保存

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/094_cancellable_file_exports.md`

## 目标

让正在进行的图片文件保存可从既有系统 tray 取消，并保证取消只影响仍在运行的 `SaveImage` 任务。取消前已编码但尚未原子发布的临时文件必须清理；已经完成原子发布的目标文件不作反向删除。

## 非目标

- 不取消图像复制、OCR 文本复制、OCR、历史读取、截图或任何其他 owner 的后台任务。
- 不新增进度窗口、通知、文件管理器启动、外部进程、线程类型、设置 schema、导出目录、命名模板或覆盖策略。
- 不承诺百分比进度，也不把“取消请求已送达”误报为已删除已经原子发布的目标文件。

## 依赖关系

- 依赖 `JobSupervisor::cancel` 的协作式 token 与既有 `ExportJobService` worker 收敛。
- 依赖 028/092 的同目录临时文件、编码、同步和原子发布；取消检查必须落在发布前的可回滚阶段。
- 依赖 078 的 tray 状态反馈与 tray-only 约束。

## 约束

- tray 仅在至少存在一个运行中的 `SaveImage` 时启用“取消文件保存”；一次操作取消所有当前运行的文件保存，绝不取消 clipboard 类型任务。
- 主线程发出取消后保持 job/pending 映射，直到 worker 结果收敛；菜单根据真实 `JobState::Running` 更新，陈旧菜单动作安全无副作用。
- worker 必须在编码前、临时文件发布前和完成前检查 token；发布前发现取消时只留下由当前任务创建的临时文件清理结果，不修改历史索引或既有目标。
- 原子 rename 已完成后文件属于真实外部副作用，取消不得删除或伪称回滚成功；最终反馈必须区分“正在取消”和“已取消”。
- tray 文案、日志与诊断不包含图像、OCR 文本、路径、内部 ID 或原始错误；不新增窗口、展示入口或任务栏/Dock/分页器项。

## 阶段

1. 扩展受监督保存的取消检查和服务单任务取消入口，覆盖临时文件与发布边界。
2. 扩展 tray 动作、禁用状态和脱敏反馈，在桌面壳中只定位运行中的文件保存。
3. 覆盖服务取消、临时文件清理、clipboard 隔离、tray 映射/状态和桌面壳选择逻辑。
4. 运行定向、workspace、跨 target、严格静态、差异和上下文门禁；将实际慢盘、查看器和原生 tray/窗口行为记录为风险。

## 检查点

1. 无运行文件保存时 tray 取消项禁用；启动保存后启用，发出取消请求后禁用并直到 worker 收敛才移除 pending。
2. 取消只命中运行中的 `SaveImage`；CopyImage/CopyText 的 job、反馈和结果不受影响。
3. 发布前取消不留下临时文件、不写历史、不替换既有目标；已发布目标不因迟到取消被删除。
4. 所有反馈为静态文案，tray-only 的窗口创建/展示边界与既有全量回归继续通过。

## 计划级风险

- 编码或 `sync_all` 本身不可随意中断；取消只能在协作检查点生效。极大图或慢盘的取消等待时间、原生 tray 动态刷新和文件系统语义须在桌面会话验证。
- 原子发布和取消发生在极窄竞态窗口时，系统必须保留已发布文件而不能声称回滚；需以稳定反馈明确边界。

## 变更前记录

```text
目的：补齐文件导出的可取消状态，使长时间保存不再只能等待或关闭 owner。
影响路径：ExportJobService、图片原子保存、tray action/菜单状态、tray feedback、DesktopApp pending export 轮询与上下文风险。
兼容性：现有格式/质量、命名、路径、历史、clipboard、owner/generation、PinId、设置、权限和窗口策略不变。
外部副作用：用户从 tray 取消当前文件保存；取消前发布的临时文件会清理，已发布文件保留；不联网、不新增窗口、不请求权限。
回滚点：删除 tray 取消项、服务单任务取消和保存检查点，恢复仅 owner 关闭/退出时取消的行为。
验证场景：菜单启停、单/多文件保存、取消隔离、临时文件清理、发布边界、陈旧 action、反馈、历史、窗口策略和 workspace 门禁。
```

## 完成标准

- 用户可从 tray 取消当前运行的文件保存，clipboard 任务与其他后台工作不受影响。
- 发布前取消不会遗留当前任务的临时文件或写入历史；已发布文件不被取消路径删除。
- 定向、workspace、跨 target、严格静态、差异和 `ctx validate` 通过；真实性能、文件系统、tray 和窗口管理器行为如实保留为风险。

## 完成记录

- 已实现：`ExportJobService::cancel` 代理既有单任务协作式取消；本地图片保存于编码前、编码后及原子发布前检查 token。发布前取消由 `AtomicExportTemp::Drop` 清理当前任务临时文件，原子 `rename` 后不删除目标。
- 已实现：tray 新增默认禁用的“取消文件保存”，仅在存在运行中的 `SaveImage` 时启用；点击仅取消这类任务，pending 映射保持到 worker terminal 结果。固定反馈区分“正在取消文件保存”与“文件保存已取消”，不携带路径或用户内容。
- 已验证：`cargo test -p pinora-app --lib image_sink::tests -- --nocapture`（13 通过、1 忽略）、`export_job::tests`（12 通过）、`tray::tests`（14 通过）、`tray_feedback::tests`（4 通过）、`desktop_shell::`（37 通过）和 `window_policy::`（4 通过）。
- 已验证：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 277 通过、2 忽略；core 89 通过）、`cargo check --workspace --target x86_64-pc-windows-msvc`、`git diff --check` 与 `ctx validate` 均通过。
- 未验证：真实慢盘/网络挂载磁盘取消延迟、最后检查与 `rename` 的文件系统竞态、Windows/macOS/X11/KDE Wayland tray 动态刷新、真实查看器与任务栏/Dock/分页器隔离；已登记为 `R-052`。
