# 计划 082：OCR 语言设置与 schema v2 迁移

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/082_ocr_language_settings.md`

## 目标

为本地 Tesseract OCR 增加可持久化、可在设置窗口切换且实际影响 worker 的语言预设。设置文件从 schema v1 迁移到 v2 时必须保留既有四个字段并默认新增语言为自动；缺失的指定模型必须受控失败，不能默默改用其他语言或联网下载。

## 非目标

- 不实现任意语言字符串、模型目录、模型下载、置信度阈值、OCR 超时配置、结果缓存、重试 UI、云端 OCR 或 OCR 文本上传。
- 不改动 OCR owner/generation 门禁、worker 取消/超时/输出上限、历史格式、导出、截图、热键、窗口策略或系统权限语义。
- 不改变 v1 文件的四个既有字段解释；不记录用户路径、像素或 OCR 正文到设置文件、日志或上下文。

## 依赖关系

- 依赖 `pinora-core::AppSettings`、`SettingsStore` 的原子替换、`SettingsPanel` 草稿事务、`OcrJobService` 和既有 Tesseract CLI 适配器。
- 依赖 081 的 tray-only 主事件循环；设置仍通过 `window_policy` 的唯一展示入口打开。
- 后续模型目录、任意语言、阈值、超时、缓存与重试必须在本任务的稳定枚举和 worker 参数边界上单独扩展。

## 约束

- schema v1 必须可读取为当前设置，新增字段采用安全默认值并标记迁移；保存只写 schema v2 的固定记录。
- 语言预设仅允许 `Auto`、`English`、`SimplifiedChinese`。自动模式仅从本机可见模型中选择；指定模式缺少对应模型时返回 `CapabilityUnavailable`。
- OCR worker 必须在提交时冻结语言选项；保存新设置只能影响之后提交的任务，不能改变运行中或已完成 OCR 结果。
- 所有新设置交互在现有 settings window 内完成，不创建额外窗口、事件循环、worker 或系统进程；保存失败不得改变 runtime 设置。

## 阶段

1. 在核心模型定义语言枚举、默认值、逐字段修复与 schema v2，并建立 v1/v2 编解码迁移测试。
2. 扩展设置面板的语言行与键盘/鼠标状态机；设置保存后只影响后续 OCR 提交。
3. 让 `OcrJobService` 显式接收冻结的语言预设，Tesseract 仅使用本机安装模型并返回稳定的受限能力错误。

## 检查点

1. schema v1 文件加载后保留原值、默认语言为自动，并在下一次保存输出有效 schema v2。
2. English/简体中文预设精确选择要求的模型；自动模式保持既有“优先中英，否则可用语言”的本地行为。
3. 保存语言设置不影响运行中 OCR，新增任务将携带提交时的预设；无模型时不会伪造 OCR 成功。

## 完成标准

- 核心、codec、设置面板、OCR 语言选择和 job 冻结分别有纯逻辑或服务契约测试。
- v1 迁移、v2 往返、无效 wire、缺模型、可选模型选择和运行中任务边界均受控。
- 严格 workspace 门禁、Windows target 编译、上下文校验与差异检查通过；真实 Tesseract 模型可用性和 GUI 输入仍与桌面环境分开记录。

## 计划级风险

- `tesseract --list-langs` 和系统路径在不同平台可用性不同；本任务只能验证选择逻辑和受控失败，不能宣称任一模型已安装。
- 现有自绘设置窗口仍缺输入法、读屏和原生主题；本任务不把新增一行控件表述为完整生产级设置 UX。

## 变更前记录

```text
目的：让用户能持久化并实际控制本地 OCR 使用的语言预设，同时安全迁移已有设置文件。
影响路径：核心设置模型、固定长度 codec、设置面板、OCR runner/job 提交、desktop shell、测试与上下文。
兼容性：v1 设置字段保持原语义；新增 schema v2 字段默认 Auto；不改变截图、历史、导出、权限或状态字符串。
外部副作用：后续 OCR 子进程将按已保存预设传递本机模型名称；不下载模型、不访问网络、不写入用户指定目录。
回滚点：读取 v2 时忽略新增语言字段并始终使用 Auto；保留 v1 解码、既有任务监督和设置原子保存。
验证场景：v1 迁移、v2 往返、无效 wire、面板切换、worker 冻结、模型缺失、取消/超时回归与 workspace 门禁。
```

## 完成记录

- `OcrLanguage` 以稳定 wire 值 `Auto=0`、`English=1`、`SimplifiedChinese=2` 写入 schema v2；18 字节 v1 记录在内存中迁移为 v2 默认 `Auto`，下一次原子保存写出 19 字节 v2 记录。
- 设置面板在既有窗口中提供三种预设的键盘/鼠标循环。成功保存才更新 runtime，任务启动时由 `desktop_shell` 读取当前值并传给 `OcrJobService`；worker 闭包持有该副本，运行中任务不会读后续设置。
- 自动模式只组合本机 `chi_sim`、`eng`，指定模式要求精确模型；缺失时返回 `CapabilityUnavailable`，不下载模型、不回退到其他语言。OCR 失败日志只写稳定错误码。
- 已验证：设置模型、v1/v2 codec、面板、模型选择、job 冻结、Overlay 契约、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`。真实 Tesseract 模型组合、设置窗口输入、系统剪贴板和四类原生桌面仍未验收。
