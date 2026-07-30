# 任务 002：细化增强开发设计文档

- 状态：已完成
- 计划：`.context/plans/002_design_document.md`
- 规模：大
- 依赖：`.context/plans/001_context_bootstrap.md`
- 生产行为变更：无

## 任务目标

重写 `docs/Pinora-开发设计文档.md`，形成覆盖功能、模块、架构、状态、流程、验收和风险的 Mermaid 驱动设计基线。

## 范围

- 细化截图、选区、贴图、标注、OCR、热键、托盘、设置、剪贴板/导出、历史和平台适配功能。
- 增加模块总览图、软件架构图、领域状态图、启动/截图/贴图/OCR/热键/导出流程图。
- 增加优先级、验收标准、非功能指标、测试策略、发布阶段和待决策项。

## 非目标

- 不修改 `src/`、`Cargo.toml` 或运行时行为。
- 不验证或锁定 GPUI、Liora、xcap、ashpd、OCR 引擎的具体版本。
- 不创建数据库、云端服务或真实平台权限连接。

## 预期文件

- 修改 `docs/Pinora-开发设计文档.md`。
- 新增本计划和本任务上下文文件。
- 更新 `AGENTS.md` 的当前计划/任务指针和 `.context/system/overview.md` 的设计基线描述。

## 验收标准

- 文档包含每个核心功能的详细行为契约、失败路径和验收标准。
- 文档包含至少一张功能模块总览 Mermaid 图、一张软件架构 Mermaid 图和六类以上流程/状态图。
- 所有图表位于 `mermaid` fenced code block 中，节点命名与模块章节一致。
- 明确区分已验证仓库事实、目标设计和待确认项。

## 验证

- 使用脚本统计 Mermaid 围栏是否成对、图类型是否存在、文档关键章节是否齐全。
- 运行 `python /home/neo/.claude/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`。
- 运行 `git diff --check`；不运行会访问外部基础设施的命令。

## 风险与回滚

- 风险：Mermaid 渲染器对复杂中文标签或特殊字符解析不一致。缓解：使用标准图类型、短节点 ID 和引号标签；在渲染平台复核。
- 风险：详细设计被误读为已实现。缓解：文档元数据与章节反复标注“目标设计”，并同步 `.context/system/overview.md`。
- 回滚：恢复 `docs/Pinora-开发设计文档.md` 旧版本，恢复 `AGENTS.md` 及上下文指针到 `001`；不涉及源码。

## 完成记录

- 状态：已完成（2026-07-30）。
- 实际变更：完成设计文档重写，新增功能规格、模块边界、领域模型、架构图、状态图、时序图、验收、测试和发布规划。
- 实际验证：结构脚本确认文档 1069 行、17 个 `mermaid` 图块、代码围栏全部闭合且关键章节齐全；`context_bootstrap.py validate` 返回 `ok: true`、无错误无警告；尾随空白检查通过。
- 未解决项：第三方依赖版本、真实平台能力和图表最终渲染效果仍需在实现阶段验证。
