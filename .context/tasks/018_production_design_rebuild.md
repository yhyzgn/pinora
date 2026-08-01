# 任务 018：重写生产级开发设计基线

- 状态：已完成
- 计划：`.context/plans/018_production_design_rebuild.md`
- 规模：大
- 依赖：`.context/tasks/016_takeover_audit.md`、`.context/tasks/017_rust_quality_baseline.md`
- 生产行为变更：无

## 目的

细化 `docs/Pinora-开发设计文档.md`，使其能够约束后续对 Grok 遗留实现的替换，而不是继续为单体桌面壳和未验证技术方案背书。

## 任务目标

建立一个可直接拆分为重构任务的中文设计基线：每个核心功能有详细规格和验收，模块/架构/流程均以 Mermaid 表达，且所有已验证现状、目标设计和待验证决策可追溯区分。

## 影响路径

- `docs/Pinora-开发设计文档.md`
- `AGENTS.md` 当前工作指针
- `.context/plans/018_production_design_rebuild.md`
- `.context/tasks/018_production_design_rebuild.md`
- `.context/system/overview.md`（设计基线状态）

## 兼容性

- 接口：无运行时代码或公共接口变更。
- 数据：无持久化数据形状变更；只定义未来迁移规则。
- 状态：无运行时状态字符串变更。
- 租户/权限：不涉及租户；文档明确平台权限和本地隐私边界。

## 外部副作用

无。不启动图形桌面、不请求截图/热键权限、不连接外部服务。

## 回滚点

反转本任务中的文档和上下文差异即可；不影响源码行为。

## 验证场景

- 文档元数据、现状/目标/待决策边界可区分。
- Mermaid 围栏成对，含模块总览、架构、状态、时序或流程图。
- 每个核心模块可追溯到入口、主流程、失败语义、数据边界和验收条件。

## 范围

- 修正当前代码状态与已验证平台能力的陈述。
- 增强功能模块总览、架构边界、任务控制、数据生命周期、平台能力矩阵和迁移顺序。
- 增加或修订 Mermaid 图以覆盖生产重构关键流程。

## 非目标

- 不实施代码重构、依赖升级或 GUI 改版。
- 不承诺或伪造 Windows、macOS、Linux Wayland 的能力实现。
- 不删除遗留模块或改动历史数据。

## 预期文件

- `docs/Pinora-开发设计文档.md`
- `AGENTS.md`
- `.context/plans/018_production_design_rebuild.md`
- `.context/tasks/018_production_design_rebuild.md`
- `.context/system/overview.md`

## 验收标准

- 设计文档把“当前实现”“目标架构”“待验证决策”明确隔离。
- 所有核心功能模块有具体详细功能、异常处理和验收标准。
- Mermaid 图覆盖模块总览、软件架构、截图、贴图、标注、OCR、导出、任务生命周期与迁移流程。
- 不出现不受证据支持的已实现或跨平台声明。

## 验证

- 使用 `awk` 检查 Markdown fenced code block 成对，并统计 Mermaid 图块。
- 检查 0-12 章节、4.1-4.10 功能规格和关键流程标题存在。
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`。
- `git diff --check`。

## 风险与回滚

- 风险：复杂目标文档可能被误读为当前实现。缓解：元数据、第 0 节和平台矩阵持续区分事实、目标与待验证项。
- 风险：Mermaid 在不同渲染器存在兼容性差异。缓解：使用标准图类型、短节点 ID、成对围栏检查和后续渲染复核。
- 回滚：反转本任务的文档和上下文差异；不影响源码、依赖、用户数据或外部系统。

## 完成记录

- 状态：已完成（2026-08-01）。
- 初始证据：016 已确认桌面壳单体化、Linux/KDE 绑定、fake 截图回退和 OCR/剪贴板生命周期缺口；017 已恢复 Rust 静态质量门禁。
- 实际变更：设计文档修正了过期的“无实现”与具体 UI 框架前提，使用已验证现状/目标设计/待验证决策三层边界；细化 12 个目标模块、核心功能失败语义、资产 generation、任务取消、系统剪贴板真实成功语义、平台矩阵及受控迁移顺序。
- 验证：22 个 Mermaid 围栏成对且无错误；关键章节与 4.1-4.10 功能规格存在；`context_bootstrap.py validate` 返回 `ok: true`；`git diff --check` 通过。
- 风险：图表的最终浏览器渲染需在目标文档平台复核；具体 UI/平台/OCR 技术仍需后续研究和最小探针。
