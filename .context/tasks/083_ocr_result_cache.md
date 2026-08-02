# 任务 083：OCR 结果缓存与重复识别消除

- 状态：已完成
- 计划：`.context/plans/083_ocr_result_cache.md`
- 规模：中
- 依赖：082 的 `OcrLanguage` 冻结、现有 `OcrJobService`、`JobSupervisor` 和 `desktop_shell`。
- 生产行为变更：同一运行期内重复 OCR 同一资产版本和语言时可直接使用已验收内存结果，避免重新启动本地 OCR 子进程。

## 任务目标

为 `OcrJobService` 增加有界、非持久化的结果缓存，并在 `desktop_shell` 发起 OCR 前使用它。任何缓存条目只能来自 accepted completion，缓存命中必须复用现有结果交付、全文复制和 tray 反馈路径。

## 范围

- 在 `crates/pinora-app/src/ocr_job.rs` 实现以 `AssetRef`、`OcrLanguage` 为键的有界结果缓存与服务查询入口。
- 将 `desktop_shell` 中 OCR 成功处理提取为共享函数，未命中走既有 worker，命中走同一交付函数。
- 增加缓存命中、隔离、失败拒绝、容量和 clone 语义测试；更新稳定事实、风险和完成记录。

## 非目标

- 不做磁盘缓存、跨启动/跨进程复用、内容哈希、任务去重、配置 schema、模型/引擎版本发现、置信度筛选或 OCR 重试界面。
- 不改动本地 Tesseract 模型选择、OCR 语言、worker 取消/超时、截图、导出、历史、热键、窗口策略或发布流水线。

## 预期文件

- `crates/pinora-app/src/{ocr_job.rs,desktop_shell.rs}`
- `AGENTS.md`
- `.context/plans/083_ocr_result_cache.md`
- `.context/tasks/083_ocr_result_cache.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 同一 `AssetRef` 和 `OcrLanguage` 的 accepted 成功结果可读取；不同语言、不同 generation、失败、取消、超时和关闭 owner 都无法得到缓存结果。
2. 缓存有固定条目与估算字节上限，超限结果不写入，淘汰不会影响已交付结果，读取副本不能修改缓存。
3. `desktop_shell` 在命中时不提交新的 OCR worker，仍更新文字层、提交全文复制并设置受控 tray 反馈；未命中保持既有异步/取消/陈旧门禁。
4. 不新增窗口、事件循环、网络、模型下载或持久化 OCR 文本，tray-only 约束不变。

## 验证

- `cargo test -p pinora-app ocr_job -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app ocr -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：缓存键遗漏 generation 或语言会交付陈旧/错误文字层。缓解：键只使用完整 `AssetRef` 与已冻结 `OcrLanguage`，并为隔离和 rejected completion 建立测试。
- 风险：无界 OCR 文本缓存会导致 tray 常驻内存长期增长。缓解：固定条目数、估算字节上限和不缓存过大结果；真实内存峰值仍需实机测量。
- 回滚：删除 cache 存储/查询和共享命中分支，所有请求恢复既有 worker；不修改设置、资产、历史和窗口行为。

## 完成记录

- 完成时间：2026-08-02。
- 交付：`OcrJobService` 以完整 `AssetRef` 和 `OcrLanguage` 缓存 accepted 成功 `OcrResult`；`start_or_reuse` 在 service 内决定命中或新 worker。缓存上限为 8 条、2 MiB 估算总量和 512 KiB 单条估算，进程结束后自动释放。
- 交付路径：`desktop_shell` 的缓存命中与 worker 完成共用结果交付函数，仍更新匹配贴图词框、使用既有 `ExportJobService` 复制全文并更新 tray；命中不启动 `tesseract` worker，也不创建窗口或网络请求。
- 已验证：`cargo test -p pinora-app ocr_job -- --nocapture`（13 项）、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`（30 项）、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（应用 219 项、核心 86 项，2 项真实桌面测试跳过）以及格式、workspace 编译、严格 Clippy 均通过。
- 未覆盖风险：缓存不感知运行中外部模型文件变化；真实连续点击、4K OCR 结果内存、系统剪贴板、设置窗口、任务栏/Dock/分页器和四类原生桌面时延仍需实机验收。
