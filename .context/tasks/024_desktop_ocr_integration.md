# 任务 024：接入桌面 OCR 任务监督

- 状态：已完成
- 计划：`.context/plans/024_desktop_ocr_integration.md`
- 规模：大
- 依赖：`.context/tasks/023_ocr_job_service.md`
- 生产行为变更：是；OCR 从 UI 同步/裸线程执行改为后台受监督任务，关闭 owner 后晚到结果被丢弃。

## 目的

修复 `desktop_shell` 中贴图 OCR 阻塞 UI、Overlay OCR 裸线程脱离生命周期的问题，将两个入口统一到已验证的应用 OCR 服务。

## 任务目标

为每个贴图绑定 `AssetRef::initial(image.id)` 和 `JobOwner::Pin(pin_id)`；为每个 Overlay 分配 `SessionId` 并保存当前 OCR 资产。`O` 键和工具栏 OCR 只提交 `JobSpec`，`about_to_wait` 轮询 `OcrJobCompletion` 后才更新词框/复制全文。贴图关闭、Overlay 取消/再截、应用退出均取消对应 owner。

## 影响路径

- `crates/pinora-app/src/desktop_shell.rs` 的状态、事件循环、OCR 入口和关闭路径。
- `crates/pinora-app/src/ocr_job.rs` 的公共服务使用方式（不改变其协议）。
- 当前计划、任务、系统概览和风险登记。

## 兼容性

- 接口：保留用户已有 `O` 键、工具栏 OCR、全文复制和贴图词框显示；删除内部同步调用，不改变领域命令字符串。
- 数据/状态：不改变持久化或 `AppState` 数据形状；新增进程内 owner/asset 字段。
- 生命周期：窗口事件只提交任务；真实 OCR 子进程仍由适配器持有和回收。

## 外部副作用

离线测试不启动桌面壳；workspace 中既有本机 Tesseract 冒烟仍按环境运行。不得连接网络或共享基础设施；真实桌面探针继续保持 ignored/显式授权。

## 回滚点

回滚 `desktop_shell` 的服务字段、owner/asset 字段和 OCR 入口改动即可；保留 023 服务与 022 进程修复，不回退领域契约。

## 验证场景

- 贴图按 `O` 后事件循环继续处理窗口/热键，完成后词框更新并复制全文。
- Overlay 工具栏 OCR 完成后复制全文；取消或再截后晚到结果被丢弃。
- 贴图关闭后 OCR 结果不会重新创建窗口或写入 `PinWin`。
- OCR 失败、取消、超时只记录受控诊断，不导致事件循环退出。

## 范围

- DesktopApp 持有 `OcrJobService`，轮询并应用完成/失败/丢弃结果。
- Pin/Overlay owner 与资产引用绑定，所有关闭路径取消任务。
- 删除旧 OCR 直接调用，补静态扫描和相关离线测试。

## 非目标

- 不实现 OCR 文字层拖选编辑、进度条、结果缓存或剪贴板适配器重构。
- 不拆分 `desktop_shell` 其他窗口绘制和标注逻辑。
- 不把无 GUI 的服务测试描述为真实窗口 E2E。

## 预期文件

- `crates/pinora-app/src/desktop_shell.rs`。
- 必要时 `crates/pinora-app/src/ocr_job.rs`、`crates/pinora-app/src/job_supervisor.rs`。
- 当前计划、任务、`.context/system/overview.md`、`.context/system/risks.md`、`AGENTS.md`。

## 验收标准

- `desktop_shell.rs` 不再包含直接 `recognize_image` 调用和 OCR 专用裸 `thread::spawn`。
- OCR 结果处理经过服务的 owner/generation/终态校验；关闭路径调用 owner 取消。
- workspace 静态质量门禁和离线回归通过，真实桌面缺口明确记录。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app ocr_job::tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `rg -n 'recognize_image|thread::spawn' crates/pinora-app/src/desktop_shell.rs`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：无 GUI 环境无法验证窗口重绘和焦点。缓解：保持纯服务测试、编译门禁和真实桌面探针缺口，不宣称 E2E。
- 风险：Overlay 只保留全文复制，用户不会看到新增的可编辑文字层。缓解：明确不扩大本任务产品语义，后续 OCR 文字层单独设计。
- 回滚：恢复旧 OCR 入口即可，不删除 022/023 的新适配器和服务。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：`desktop_shell.rs::run_pin_ocr` 在事件处理线程同步调用 `recognize_image`；`overlay_ocr` 创建无 owner 的 `thread::spawn`；`close_pin`、`cancel_overlay` 和 `request_new_capture` 未取消 OCR 任务。
- 实际变更：`DesktopApp` 持有 `OcrJobService` 并在事件循环中轮询；`PinWin` 绑定贴图资产引用，`OverlayState` 绑定会话 owner 与 OCR 资产；OCR 入口只提交 `JobSpec`，worker 不接触窗口、应用状态或剪贴板。关闭/取消/再截/退出路径补齐 owner 关闭与全量取消，结果经终态、owner 和 generation 门禁后才交付。
- 验证：`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（app 60 项通过、core 39 项通过，2 个真实桌面测试忽略）、`rg -n 'recognize_image|thread::spawn' crates/pinora-app/src/desktop_shell.rs`、`git diff --check` 与上下文校验通过。
- 未覆盖项：无 GUI E2E；退出只保证取消信号发出，尚无 worker join/收敛等待；系统剪贴板和导出仍采用旧生命周期；复杂 OCR 子孙进程回收需后续平台验证。
