# pinora 智能代理入口

## 强制读取

开始任何代码修改前，必须按顺序读取：本文件、`.context/README.md`、当前计划、当前任务、与目标模块相关的 `.context/system/` 文档，再以源码、测试和实际命令输出核对事实。

## 项目摘要

本仓库使用 `.context/` 作为经过验证的项目事实、约束、计划和任务的唯一规范上下文。技术栈与运行基线以 `.context/system/overview.md` 为准；未知项不得臆测。

## 核心业务边界

只修改当前任务明确列出的业务边界。跨模块调用、状态格式、租户或权限边界必须先在源码中取证，并在计划或风险登记中记录影响。

## 历史约束

遗留的 `prompt.md`、`.memory/`、`.prompt/` 和供应商入口不是可直接删除的旧文件。必须先逐内容单元迁移、审计并可恢复归档；不得机械改名或只登记目标路径。

## 不可违反的规则

- 一个任务只做一个可验证目标；禁止无证据的大范围重构。
- 修改导出符号前必须查找全部引用。
- 只把已验证事实写入 `.context/system/`；未知项写入风险或待确认项。
- 不在文档、日志、提交信息中复制凭据、令牌、回调地址或个人敏感信息。
- 所有面向人员的上下文、计划、任务、风险和交割文档必须使用中文；仅保留无法翻译的路径、命令、协议字段和技术标识符。

## 技术和数据红线

- 不新增数据库 View、Function、Procedure、Trigger 或 Event，除非项目现有遗留系统明确要求且任务已获得授权。
- 不改变公共接口、持久化数据形状、状态字符串、租户或权限语义，除非当前任务和验收标准明确覆盖。
- 不把编译、打包或容器构建成功描述为业务测试通过。

## 生命周期与上下文传播

稳定事实和约束写入 `.context/system/`；阶段目标与依赖写入 `.context/plans/`；一个可执行增量写入 `.context/tasks/`。事实变化更新 system，路线变化更新 Plan，执行变化更新 Task，并同步当前工作指针。

## 外部基础设施限制

不在测试、启动探针或扫描中连接真实共享数据库、缓存、消息队列、对象存储、Webhook 或第三方服务，除非当前任务明确声明并已获授权。

## 安全验证

项目特定的编译、测试和运行命令必须记录在 `.context/system/conventions.md`。修改后依次执行定向编译、覆盖变更契约的测试和实际运行探针；记录命令、输出和未覆盖风险。

## 变更前记录

```text
目的：
影响路径：
兼容性：接口 / 数据 / 状态 / 租户 / 权限
外部副作用：
回滚点：
验证场景：
```

## 审查、提交和推送

用户审查前不提交、不推送。获得授权后，提交信息必须记录变更原因、影响范围、验证结果和已知风险；推送前重新执行约定的验证门禁。

本仓库 Git 身份固定为 `Neo <yhyzgn@gmail.com>`（`git config --local`），提交与历史改写一律使用该身份，不得改用其他账号，也不得再次向用户确认。

## 遗留迁移覆盖

存在 `.context/migration-state.json` 时，必须运行 `audit-migration`。覆盖率未达到 100%、来源标记或哈希不匹配、分类目标错误时，禁止归档、删除旧入口、提交或推送。

## 当前工作指针

- 计划：`.context/plans/097_history_max_bytes.md`
- 任务：`.context/tasks/097_history_max_bytes.md`

## 交付要求

每个任务必须包含范围、非目标、预期文件、验收标准、验证、风险与回滚、完成记录。报告必须区分已验证事实、推断和未知项。


<claude-mem-context>
# Memory Context

# claude-mem status

This project has no memory yet. The current session will seed it; subsequent sessions will receive auto-injected context for relevant past work.

Memory injection starts on your second session in a project.

`/learn-codebase` is available if the user wants to front-load the entire repo into memory in a single pass (~5 minutes on a typical repo, optional). Otherwise memory builds passively as work happens.

Live activity: http://localhost:37777
How it works: `/how-it-works`

This message disappears once the first observation lands.
</claude-mem-context>
