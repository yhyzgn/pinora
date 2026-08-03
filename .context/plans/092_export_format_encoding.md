# 计划 092：导出格式与编码质量

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/092_export_format_encoding.md`

## 目标

将导出能力从唯一 PNG 扩展为用户可选择的 PNG、JPEG 与无损 WebP，并为 JPEG 提供可持久化的质量设置。每个格式都必须沿用既有受监督 worker、同目录临时文件、同步、原子发布、owner/generation 门禁、脱敏反馈与文件名冲突分配，不以格式扩展引入主线程编码或新的窗口生命周期。

## 非目标

- 不实现历史索引的 JPEG/WebP 读取、缩略图或编辑；本任务中只有 PNG 成功保存继续进入既有 PNG-only 历史索引，非 PNG 结果不伪造为已记录。
- 不实现自定义命名模板、目录选择、覆盖/取消策略、打开文件位置、导出进度 UI、质量预览或 WebP 有损质量调节。
- 不改变系统图像剪贴板：它继续发送 PNG，不受文件导出格式设置影响。
- 不新增窗口、事件循环、外部进程、网络访问、权限请求或真实桌面 E2E 声明。

## 依赖关系

- 依赖 028 的原子 PNG 发布、026/027 的 `ExportJobService` 与 owner/generation 门禁。
- 依赖 080 的受控 UTC 文件名分配和 037 的 PNG-only 历史候选机制。
- 依赖 091 的版本化设置、原子回读和仅在保存成功后应用 runtime 的规则。
- 编码使用 `image` 0.25（当前 lock 为 0.25.10），关闭直接依赖的默认特性，仅启用 `jpeg`、`webp`：官方文档确认 JPEG `new_with_quality` 接收 1..=100，WebP 提供无损 RGBA 编码；许可证 MIT/Apache-2.0。

## 约束

- 新增 `ExportImageFormat::{Png,Jpeg,WebP}`；默认 PNG。JPEG 质量只在 1..=100 有效，默认 90；PNG/WebP 忽略该值但继续安全保存。
- 设置 schema v6 在 v5 尾部追加格式和 JPEG 质量；v1-v5 必须保留原字段并以默认格式/质量迁移，非法枚举拒绝，非法质量逐字段修复，保存继续原子回读。
- JPEG 不支持 alpha，编码前必须以不透明白色确定性合成 RGBA；PNG 和无损 WebP 保留 RGBA。不得把透明通道保留错误地表述为 JPEG 结果。
- 每个文件任务的格式、质量和路径在提交 worker 前冻结；运行中设置变更不影响已启动任务。用户选择格式必须驱动扩展名，文件名不得包含图像、OCR、路径或内部 ID。
- 保存完成反馈改为通用文件保存状态；不得把 JPEG/WebP 成功标注为 PNG，非 PNG 不得进入或破坏 PNG-only 历史索引。

## 阶段

1. 建立核心格式/质量模型与 schema v6，覆盖 v1-v5 迁移、往返和非法字段修复。
2. 以最小 `image` 编码特性扩展既有导出 worker与原子临时文件，覆盖 PNG/WebP RGBA、JPEG 白底合成、质量边界和无内容泄露的错误路径。
3. 扩展设置面板、文件名分配、桌面提交点和 tray 反馈；保持剪贴板与 PNG-only 历史边界明确。
4. 运行依赖、定向、workspace、跨 target、严格静态、差异和上下文门禁；将真实编码兼容性、性能和桌面行为记录为风险。

## 检查点

1. PNG、JPEG、WebP 分别获得正确魔数和扩展名；JPEG 质量可控且其透明输入被确定性白底合成。
2. v1-v5 设置均迁移为 PNG/90，v6 完整往返；坏格式拒绝、坏质量修复，保存失败不改变运行时。
3. 格式/质量在任务提交时冻结；同一进程中已有 worker 不读取之后的设置，系统剪贴板仍使用 PNG。
4. 非 PNG 成功保存的反馈不称 PNG，且不会写入/删除/解码 PNG-only 历史；不新增窗口、worker 类型或敏感日志。

## 计划级风险

- `image` 编码器的实机文件兼容性、JPEG 色彩/白底合成视觉与大图内存/耗时必须在原生桌面会话和目标平台文件查看器中验证；CI 与单位测试不能替代。
- JPEG/WebP 暂不进入历史将导致用户只能在当次路径中访问这些文件；多格式历史读取必须独立实现，不能放宽现有 PNG 校验。

## 变更前记录

```text
目的：补齐设计文档的 PNG、JPEG、WebP 文件导出与 JPEG 质量设置。
影响路径：核心设置模型、codec、设置面板、导出输入/worker、编码/原子文件、命名分配、桌面提交和反馈、上下文与风险。
兼容性：设置 schema v5 升至 v6，v1-v5 可读迁移。默认 PNG、系统剪贴板、领域状态、历史索引、PinId、窗口策略、权限和任务身份不变。
外部副作用：用户选择保存时写入受管本地导出目录；不联网、不创建窗口、不请求权限，不连接共享服务。
回滚点：保留 v6 decoder；移除格式设置与非 PNG 分支并将后续保存固定 PNG，既有 PNG、剪贴板和历史不变。
验证场景：三格式魔数/扩展名、JPEG alpha 合成/质量、设置迁移/回读/失败、worker 冻结、PNG 历史隔离、反馈、窗口策略和 workspace 门禁。
```

## 完成标准

- 设置可选择 PNG/JPEG/WebP 与有效 JPEG 质量，默认行为保持 PNG；旧设置安全迁移。
- 三格式文件均通过既有受监督原子导出路径；JPEG alpha 与 PNG/WebP RGBA 语义明确并有测试。
- clipboard 与 PNG-only 历史保持正确隔离，反馈不误报格式。
- 定向、workspace、跨 target、严格静态、差异和 `ctx validate` 通过；真实跨平台查看器、性能与桌面验证如实记录。

## 完成记录

已完成。

- 设置 schema 已升至 v6：27 字节记录保存导出格式与 JPEG 质量；v1-v5 保留已有字段并迁移为 PNG/90，未知格式拒绝，非法质量逐字段回退。
- `ExportJobService` 的文件输入已改为 `SaveImage`，提交时冻结路径、格式与质量；PNG、JPEG、无损 WebP 共用既有 worker、取消、owner/generation/截止时间门禁与同目录原子发布。JPEG 在 worker 内确定性白底合成 RGB，PNG/WebP 保持 RGBA。
- 设置面板、格式化文件名、文件扩展名校验与 tray 反馈均已接入；系统剪贴板仍固定 PNG，只有 PNG 可写入现有 PNG-only 历史，JPEG/WebP 不伪造索引。
- 验证通过：`cargo fmt --all -- --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 268 通过、2 忽略；core 89 通过）、`cargo check --workspace --target x86_64-pc-windows-msvc`、`git diff --check`、`ctx validate`。
- 未覆盖风险：真实 Windows/macOS/Linux 查看器的编码兼容性、JPEG 视觉/色彩、极大图性能、HiDPI、tray 刷新和任务栏/Dock/分页器隔离仍须原生桌面会话验证。
