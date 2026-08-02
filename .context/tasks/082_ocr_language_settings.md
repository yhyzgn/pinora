# 任务 082：OCR 语言设置与 schema v2 迁移

- 状态：已完成
- 计划：`.context/plans/082_ocr_language_settings.md`
- 规模：大
- 依赖：081 的 GUI 主事件循环，现有 `AppSettings`/`SettingsStore`/`SettingsPanel`/`OcrJobService`。
- 生产行为变更：设置页可选择 Auto、English、SimplifiedChinese；后续 OCR 任务按保存时冻结的预设执行，缺模型时受控失败。

## 任务目标

将 OCR 语言从隐式自动探测变为版本化的设置字段，并把该字段沿设置保存、desktop shell、OCR job 和 Tesseract 参数完整传递，保持历史 v1 设置文件可读和现有异步安全边界不变。

## 范围

- 在 `pinora-core` 添加语言枚举、schema v2 默认值和修复状态。
- 为 `SettingsStore` 添加 v1 到 v2 的读取迁移与固定长度 v2 编解码。
- 在现有设置窗口增加语言行，保存成功后更新 runtime；只影响之后启动的 OCR worker。
- 让本地 OCR 根据冻结预设从已安装语言中选择，缺模型返回稳定错误；为 job 服务保留兼容的 Auto 启动入口。
- 更新工作指针、计划/任务、稳定事实和风险。

## 非目标

- 不支持任意语言、模型目录、模型安装/下载、置信度、超时、缓存、重试、云端 OCR 或 OCR 结果文本的持久化扩展。
- 不改变设置文件位置、原子写入策略、历史协议、导出、截图、热键、tray-only 窗口策略或 worker 生命周期。

## 预期文件

- `crates/pinora-core/src/{settings.rs,lib.rs}`
- `crates/pinora-app/src/{settings_store.rs,settings_panel.rs,settings_window.rs,ocr.rs,ocr_job.rs,desktop_shell.rs}`
- `AGENTS.md`
- `.context/plans/082_ocr_language_settings.md`
- `.context/tasks/082_ocr_language_settings.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. v1 文件迁移为 v2 逻辑设置后保留原字段、默认 `Auto`；v2 保存与读取精确往返，未知 schema 或语言 wire 值受控拒绝。
2. 设置面板可通过鼠标和键盘选择三种语言预设；保存失败仍保留草稿，成功保存后后续 OCR 使用新预设。
3. `OcrJobService` 在任务开始时冻结语言；Auto、English、SimplifiedChinese 的本地模型解析与缺模型失败有测试，运行中任务不读取可变设置。
4. 没有新窗口/事件循环/网络/模型下载，OCR 的取消、超时、owner/generation 门禁和 tray-only 策略回归通过。

## 验证

- `cargo test -p pinora-core settings -- --nocapture`
- `cargo test -p pinora-app settings_store -- --nocapture`
- `cargo test -p pinora-app settings_panel -- --nocapture`
- `cargo test -p pinora-app ocr -- --nocapture`
- `cargo test -p pinora-app ocr_job -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：固定 v1 record 长度与 schema v2 迁移错误会使旧设置不可读。缓解：保留 v1 专用解码、逐字段修复和迁移回归测试；未知 schema 一律不覆盖原文件。
- 风险：缺失语言模型时用户误以为已切换成功。缓解：任务实际执行时基于本机模型校验，缺失返回 `CapabilityUnavailable` 且不发布 OCR 成功。
- 回滚：保留 v2 解码但忽略 `ocr_language`，所有新任务用 Auto；不改动 v1 数据、worker 监督或窗口策略。

## 完成记录

- 完成时间：2026-08-02。
- 交付：核心设置 schema v2 和 `OcrLanguage` wire codec；v1 读取迁移、v2 原子保存与无效 wire 拒绝；设置窗口语言行；语言在 OCR worker 创建前冻结并实际传给本地 Tesseract adapter。
- 安全边界：自动模式只用本机 `chi_sim`/`eng`，指定模式不回退、不下载；OCR 相关失败日志改为稳定 `ErrorCode`，不输出 OCR 正文、临时文件路径或 Tesseract 原始 stderr。
- 已验证：`cargo test -p pinora-core settings -- --nocapture`、`cargo test -p pinora-app settings_store -- --nocapture`、`cargo test -p pinora-app settings_panel -- --nocapture`、`cargo test -p pinora-app ocr -- --nocapture`、`cargo test -p pinora-app ocr_job -- --nocapture`、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`、格式、workspace 编译和严格 Clippy 均通过。
- 未覆盖风险：真实模型安装组合、设置窗口输入/可读性、系统剪贴板、任务栏/Dock/分页器与四类原生桌面的时延和窗口行为仍需实机验收。
