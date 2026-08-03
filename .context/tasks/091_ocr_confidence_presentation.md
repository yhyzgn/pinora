# 任务 091：OCR 置信度阈值与词框呈现

- 状态：已完成
- 计划：`.context/plans/091_ocr_confidence_presentation.md`
- 规模：中
- 依赖：082 OCR 语言设置、083 OCR 缓存、088 schema v4、现有贴图 OCR 文字层。
- 生产行为变更：是；成功保存的阈值会改变已显示 OCR 词框的告警样式，但不会改变识别或复制内容。

## 任务目标

提供持久化的 OCR 置信度阈值，并用非侵入式的贴图词框视觉状态提醒用户复核低置信词。

## 范围

- 为 `AppSettings` 添加 0..=100 的 `ocr_confidence_threshold` 与默认值、字段修复和 schema v5。
- 扩展 `SettingsStore` 为 v1-v4 兼容读取和 v5 原子编码/回读；扩展现有设置面板的行、导航、鼠标和键盘调整。
- 在独立 OCR 呈现模块中定义无副作用的词框状态；贴图绘制以当前成功提交的设置计算普通、低置信和选中词框样式。
- 保存成功后更新 runtime 并请求现有贴图重绘；保存失败时不影响 runtime 或贴图。
- 增加核心、codec、面板、呈现和 desktop shell 的回归测试，并更新上下文、风险和工作指针。

## 预期文件

- `crates/pinora-core/src/{settings.rs,lib.rs}`
- `crates/pinora-app/src/{lib.rs,ocr_presentation.rs,desktop_shell.rs,settings_panel.rs,settings_store.rs}`
- `AGENTS.md`
- `.context/plans/091_ocr_confidence_presentation.md`
- `.context/tasks/091_ocr_confidence_presentation.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不改 Tesseract 命令、模型目录、语言选择、30 秒超时、缓存容量、任务监督或错误码。
- 不增加 OCR 文本列表、额外 UI 窗口、持久化 OCR 结果、后台重识别或导出功能。
- 不更改截图、标注、贴图缩放/层级、tray、历史、公共接口、权限或外部服务。

## 验收标准

1. 默认阈值为 60，数值仅可为 0..=100；旧 v1-v4 设置迁移为默认值，v5 往返保持所有字段。
2. 仅已知且小于阈值的词为低置信；未知/非有限/越界置信值不误标；选中样式优先于低置信告警。
3. 阈值只影响词框呈现；`OcrResult`、全文复制、拖选复制和缓存不删除或重排任何词。
4. 设置落盘或热键重绑失败时不更新 runtime；成功保存后现有贴图被请求重绘且不创建窗口、线程或 OCR worker。
5. 现有 window policy、tray-only、OCR、设置、core 和 workspace 回归不破坏。

## 验证

- `cargo test -p pinora-core settings ocr -- --nocapture`
- `cargo test -p pinora-app settings_panel settings_store ocr_presentation desktop_shell -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：不同 Tesseract 模型的置信度不可横向保证，且真实 HiDPI/合成器下的描边视觉与帧时间未经验收。缓解：仅作本地提示，保持无重识别和无新增窗口，并把原生验证列为风险。
- 风险：schema v5 或保存过程错误造成设置丢失。缓解：保留 v1-v4 decoder、值修复、临时文件原子替换和回读；失败不应用 runtime。
- 风险：保存后未重绘既有贴图或误影响复制。缓解：由现有 UI 线程原地 request redraw；状态为纯派生值并用测试锁定。
- 回滚：删除面板/呈现入口和保存后的 redraw；保留 v5 decoder 或将其读值忽略，以保持用户设置文件可读。

## 完成记录

- 已实现 schema v5 的 OCR 置信度阈值：默认 60，0..=100 有效，v1-v4 读取以默认值迁移；v5 往返、越界修复、损坏布尔字段保留源文件与原子回读均有回归测试。
- 设置面板新增 OCR CONFIDENCE 行；成功持久化后才应用 runtime 并请求已有贴图重绘。词框使用纯三态呈现：选中优先、已知低于阈值为告警、未知/非有限/越界为普通；不改 OCR 文本、选择、复制、缓存或 worker。
- 验证：`cargo fmt --all -- --check`；核心 88 项、设置 29 项、呈现 3 项、运行时传播 1 项定向测试；`cargo check --workspace`；严格 Clippy；`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 258 通过、2 忽略；core 88 通过）；Windows target 编译；`git diff --check` 与 `ctx validate`。真实模型、HiDPI、帧时间、tray 和任务栏/Dock/分页器行为仍是 `R-049` 的未覆盖风险。
