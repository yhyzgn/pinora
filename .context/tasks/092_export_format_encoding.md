# 任务 092：导出格式与编码质量

- 状态：已完成
- 计划：`.context/plans/092_export_format_encoding.md`
- 规模：大
- 依赖：026/027/028 导出任务与原子保存、037 PNG 历史、080 命名、091 schema v5 设置。
- 生产行为变更：是；用户可选择 PNG/JPEG/WebP 文件格式和 JPEG 质量，现有默认 PNG 不变。

## 任务目标

在不改变系统剪贴板或 PNG-only 历史语义的前提下，提供三格式受监督文件导出和可靠的 JPEG 质量/透明度处理。

## 范围

- 在 `pinora-core` 定义格式和 JPEG 质量的安全设置值，并将设置 schema 升至 v6、兼容读取 v1-v5。
- 添加最小 `image` JPEG/WebP 编码特性；把既有 `SavePng` 文件输入泛化为冻结格式/质量的 `SaveImage`，复用 worker、任务门禁和原子文件发布。
- 扩展 `image_sink` 的编码和保存：PNG/WebP 使用 RGBA，JPEG 将 RGBA 白底合成为 RGB；添加魔数、质量和原子失败测试。
- 让文件名扩展名、设置面板、桌面保存入口和 tray 保存状态跟随已冻结格式；PNG 历史候选保持唯一，非 PNG 明确不进入历史。
- 更新工作指针、稳定事实、风险和回归测试。

## 预期文件

- `Cargo.toml`、`Cargo.lock`、`crates/pinora-app/Cargo.toml`
- `crates/pinora-core/src/{export.rs,settings.rs,lib.rs}`
- `crates/pinora-app/src/{desktop_shell.rs,export_job.rs,export_name.rs,history_export.rs,image_sink.rs,settings_panel.rs,settings_store.rs,tray_feedback.rs,lib.rs}`
- `AGENTS.md`
- `.context/plans/092_export_format_encoding.md`
- `.context/tasks/092_export_format_encoding.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不新增多格式历史解码、历史 schema、任意文件浏览或导出目录/模板/覆盖/进度功能。
- 不改变 OCR、截图、标注、贴图、剪贴板、任务并发、tray-only、公共命令、权限或外部服务。
- 不将交叉编译、编码单位测试或 CI 叙述为真实平台文件查看器、窗口管理器或性能验证。

## 验收标准

1. 默认仍保存 PNG；JPEG/WebP 文件名和内容魔数正确，JPEG 质量仅为 1..=100 且透明输入确定性白底合成。
2. v1-v5 读取完整保留旧设置并默认 PNG/90；v6 往返保持新旧字段，非法格式拒绝、非法质量修复，原子保存失败不应用 runtime。
3. 每个导出任务提交时冻结格式/质量，系统图像剪贴板保持 PNG；JPEG/WebP 不被历史或 tray 伪称为 PNG。
4. 现有 PNG 导出、历史、任务门禁、设置、tray、窗口策略和 workspace 回归不破坏。

## 验证

- `cargo test -p pinora-core export settings -- --nocapture`
- `cargo test -p pinora-app image_sink export_job export_name history_export settings_panel settings_store tray_feedback -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：新增编码依赖影响三平台编译、JPEG alpha 展平或大图编码的内存/性能。缓解：仅启用 JPEG/WebP 特性，所有编码在既有 worker，保留上限/原子临时文件和像素/签名测试。
- 风险：非 PNG 不进入历史造成可发现性差异。缓解：不伪造历史写入或放宽 PNG 校验；后续单独实现多格式历史并记录该边界。
- 风险：格式标签、扩展名和字节不一致。缓解：格式枚举一处生成扩展名、编码分支和测试魔数；任务输入冻结格式与质量。
- 回滚：固定 `SaveImage` 为 PNG 并隐藏格式/质量设置，保留 v6 decoder 读取；现有 PNG、剪贴板、历史、tray 和窗口策略不变。

## 完成记录

已完成。

- 新增 `ExportImageFormat::{Png,Jpeg,WebP}` 与默认 JPEG 质量 90；设置 schema v6 兼容读取 v1-v5，坏格式保留源文件并拒绝，坏质量逐字段修复。
- 新增最小 `image` JPEG/WebP 编码特性。文件任务复用既有受监督 worker、同目录临时文件、同步和原子发布；PNG/WebP 使用 RGBA，JPEG 使用确定性白底 RGB 合成。
- 设置面板支持格式循环和 1..=100 JPEG 质量；命名、路径扩展名校验、桌面保存入口和 tray 文件保存状态都使用冻结格式。剪贴板仍为 PNG，JPEG/WebP 不进入 PNG-only 历史。
- 验证通过：`cargo test -p pinora-core export -- --nocapture`、`cargo test -p pinora-core settings -- --nocapture`、`cargo test -p pinora-app image_sink -- --nocapture`、`cargo test -p pinora-app export_job -- --nocapture`、`cargo test -p pinora-app export_name -- --nocapture`、`cargo test -p pinora-app history_export -- --nocapture`、`cargo test -p pinora-app settings_panel -- --nocapture`、`cargo test -p pinora-app settings_store -- --nocapture`、`cargo test -p pinora-app tray_feedback -- --nocapture`、完整 workspace 与跨 target 门禁。
- 风险与回滚：真实查看器兼容性、性能和桌面行为仍未验证；可隐藏格式/质量设置并固定 `SaveImage` 为 PNG，保留 v6 decoder，不影响既有 PNG、剪贴板、历史、任务门禁或窗口策略。
