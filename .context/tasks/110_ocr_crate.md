# 任务 110：OCR crate

- 状态：已完成
- 计划：`.context/plans/110_ocr_crate.md`
- 规模：中
- 依赖：任务 107 通用任务监督、任务 109 桌面交互原语。
- 生产行为变更：否；内部 crate 所有权迁移。

## 变更前记录

```text
目的：把 tesseract CLI 适配和词框呈现策略从 app 单体剥离，统一 OCR 的取消、解析和错误边界。
影响路径：workspace、pinora-app 的 OCR 任务/桌面壳导入、公共 re-export、设计文档和上下文事实。
兼容性：接口 / 数据 / 状态 / 租户 / 权限均不改变；保持语言选择、错误码、超时、输出上限和缓存门禁。
外部副作用：仍只启动调用方显式请求的本机 tesseract 子进程，并清理临时 PNG；不联网、不下载模型。
回滚点：恢复 app 内 ocr/ocr_presentation 模块和直接导入，移除 pinora-ocr。
验证场景：语言模型选择、TSV 解析、超时/取消/输出上限、临时文件 RAII、词框低置信和选择优先级。
```

## 任务目标

建立 `pinora-ocr`，让 `ocr_job` 只编排任务生命周期而不拥有 OCR 进程适配和视觉状态实现。

## 范围

- 新增 `crates/pinora-ocr/{Cargo.toml,src/{lib,ocr,ocr_presentation}.rs}`。
- 更新 workspace、app manifest、`ocr_job.rs`、`desktop_shell.rs`、app `lib.rs` 和设计/系统上下文。

## 非目标

- 不迁移 `ocr_job`、系统剪贴板、Overlay/贴图窗口、设置或模型下载。

## 预期文件

- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-ocr/Cargo.toml`、`crates/pinora-ocr/src/*.rs`
- `crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/{lib,ocr_job,desktop_shell}.rs`
- `AGENTS.md`、`.context/{plans,tasks}/110_ocr_crate.md`
- `docs/Pinora-开发设计文档.md`、`.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-ocr` 仅依赖 core/jobs、既有 png 库与标准库，拥有 OCR 适配/视觉状态的唯一实现和原有测试。
2. app 删除旧 `ocr.rs`、`ocr_presentation.rs`，兼容 OCR re-export 保持可用，`ocr_job` 只调用新 crate。
3. OCR 定向测试、workspace、严格 Clippy、Windows target、fmt、diff 和 ctx 校验通过。
4. 真实 tesseract 模型、权限、进程压力和 GUI 词框呈现缺口明确记录。

## 验证

- `cargo test -p pinora-ocr -- --nocapture`
- `cargo test -p pinora-app --lib ocr -- --nocapture`
- `cargo test -p pinora-app --lib ocr_presentation -- --nocapture`
- `cargo test -p pinora-app --lib ocr_job -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：跨 crate 的取消类型和 OCR 错误语义导入遗漏；真实本机语言模型不完整导致能力受限。
- 回滚：恢复 app 内 OCR 模块和导入，移除新 crate；不改变 OCR 数据模型、任务状态或用户文件。

## 完成记录

- `pinora-ocr` 已成为本地 Tesseract/TSV/词框视觉状态的唯一实现；依赖边界为 `pinora-core`、`pinora-jobs`、既有 `png` 和标准库。
- app 已切换到新 crate，旧 OCR 模块删除，`OcrJobService` 继续持有 owner、generation、缓存和 worker 生命周期，公共 API 保持兼容。
- 验证通过：OCR 定向 13 项、workspace 全量离线测试、workspace check、严格 Clippy、Windows target check、fmt、diff check 和 `ctx validate`。
- 已知缺口：真实引擎模型/权限、进程压力、GUI 词框与系统剪贴板未在本地离线环境验证；不联网、不下载模型、无生产数据格式变更。
