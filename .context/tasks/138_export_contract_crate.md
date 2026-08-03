# 任务 138：导出请求契约 crate

- 状态：已完成
- 计划：`.context/plans/138_export_contract_crate.md`
- 规模：小
- 依赖：任务 114、123、130、137 已完成。
- 生产行为变更：否；导出请求纯契约的 crate 边界迁移。

## 任务目标

让 `pinora-export` 唯一承载 Overlay 导出完成意图、意图到图像来源的确定性选择、导出动作分类和提交前冻结的
输出目标；让 app 保留涉及历史候选、JobState、结果资产门禁和 `TrayExportOperation` 的协调状态，避免循环依赖。

## 变更前记录

```text
目的：将无 UI、无历史的导出请求值对象从 app 私有模块提升到既有导出功能 crate，降低 desktop_shell 的导出编排密度。
影响路径：Overlay 复制、贴图、保存；贴图复制/保存；OCR 文本复制；保存格式和 JPEG 质量冻结；文件保存取消与 tray 反馈分类。
兼容性：Copy/Pin/Save 语义、CaptureExportSource、ExportImageFormat、路径、JPEG 质量、JobId、JobOwner、AssetRef、历史登记和反馈值不变。
外部副作用：无新增；文件名、worker、文件/剪贴板 IO、历史写入、Window/Surface、tray、runtime 和 EventLoop 保持原路径。
回滚点：移除 pinora-export::export_contract 并恢复 app 私有纯值对象。
验证场景：Overlay 来源选择、动作分类、冻结目标、协调层取消筛选/资产门禁/tray 映射、依赖图和全量回归。
```

## 范围

- 在 `crates/pinora-export/src/` 新增导出请求契约模块和纯逻辑回归测试。
- 将 app 的 `export_session` 收敛为仅含历史/UI/任务协调所需的 `export_coordination`。
- 更新 crate 导出、shell 导入、设计文档、系统事实、风险与工作指针。

## 非目标

- 不迁移 `PendingExport`、`HistoryExportCandidate`、取消筛选、结果资产匹配或 tray 映射到 `pinora-export`。
- 不改变导出编码、原子发布、剪贴板、历史格式、任务服务、窗口策略或 EventLoop。
- 不新增依赖、原始 SQL、警告抑制、网络访问或真实 GUI 测试。

## 预期文件

- `AGENTS.md`
- `crates/pinora-export/src/{lib,export_contract}.rs`
- `crates/pinora-app/src/{lib,desktop_shell,export_coordination}.rs`
- `.context/{plans,tasks}/138_export_contract_crate.md`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-export` 唯一拥有导出完成意图、来源选择、动作操作分类和冻结目标及对应回归测试，生产依赖不含 app、history、desktop、winit、tray 或 runtime。
2. `pinora-app` 的协调模块仅保留待处理作业、历史候选、运行中保存筛选、结果资产匹配和 tray 映射；shell 的所有真实副作用不变。
3. `Pin` 继续强制标注图，`Copy`/`Save` 继续使用当前选择，保存的路径/格式/JPEG 质量在提交前冻结。
4. 定向测试、workspace、Clippy、Windows target、版本、fmt、diff 与 ctx validate 通过。

## 验证

- `cargo test -p pinora-export -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo run --quiet -- --version`
- `cargo fmt --all -- --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：迁移可能改变贴图来源、文件保存取消范围、反馈分类或冻结输出参数。
- 回滚：删除 `pinora-export::export_contract` 并恢复 app 私有纯值对象；不触碰历史索引、导出 IO、任务协议、窗口、tray、OCR 或设置。

## 完成记录

- 已完成：新增 `pinora-export::export_contract`，迁移 `OverlayExportAction`、贴图强制标注图来源选择、
  `ExportAction`、`ExportOperation` 与 `FrozenExportTarget` 及 3 项纯逻辑回归测试。app 已删除
  `export_session`，以 `export_coordination` 保留 `PendingExport`、历史候选、运行中文件保存筛选、结果资产
  门禁和 tray 映射及 3 项协调回归测试。
- 兼容性：`Copy`/`Save` 保留当前 `CaptureExportSource`，`Pin` 继续强制 `Annotated`；保存路径、格式和 JPEG
  质量仍在提交前冻结；取消仍只选择 `JobState::Running` 的文件保存；历史登记、结果交付和 tray 反馈语义不变。
- 已验证：`cargo test -p pinora-export -- --nocapture`（33 通过，1 项真实剪贴板会话忽略）、
  `cargo test -p pinora-app --lib -- --nocapture`（10 通过）、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、
  `cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo run --quiet -- --version`、
  `cargo fmt --all -- --check`、`git diff --check`、`cargo tree -p pinora-export -e normal --depth 1` 与
  `ctx validate` 均通过。
- 风险与回滚：真实文件、系统剪贴板、tray、窗口管理器、任务栏/Dock、焦点、HiDPI 与性能仍未由离线门禁覆盖，
  R-081 持续跟踪；回滚时移除 `pinora-export::export_contract` 并恢复 app 私有纯值对象，不触碰导出 IO、
  历史索引、任务协议、窗口、tray、OCR 或设置。
