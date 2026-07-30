# 计划 003：Phase 0 可运行骨架（领域与运行时）

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/003_app_runtime_core.md`

## 目标

按设计文档 Phase 0 建立可离线验证的应用骨架：Cargo workspace、`pinora-core` 领域契约、`pinora-app` 最小 `AppRuntime`（含单实例与命令分发），以及 fake 平台能力接口。

## 非目标

- 不接入 GPUI/Liora、托盘 UI、真实热键或截图（见 D-001，另开任务）。
- 不新增第三方生产依赖，不连接外部服务。
- 不实现截图、贴图、标注、OCR 业务逻辑。

## 约束

- `pinora-core` 只依赖标准库或纯数据类型，不得依赖 UI 或平台 SDK。
- 以 `docs/Pinora-开发设计文档.md` 的模块边界与命令/事件约定为设计依据；签名保持最小可测，可后续扩展。
- 每个增量必须有可运行的 `cargo test` 证据。

## 依赖关系

- 依赖计划 001/002（上下文与设计基线已完成）。
- 不依赖真实桌面会话权限或网络。

## 阶段

1. 建立 workspace 与 `pinora-core` / `pinora-app` crate 边界。
2. 实现 Command、DomainEvent、错误码、AppState 与 AppRuntime 启动/激活/退出。
3. 用内存单实例与 fake 能力完成单元测试，并同步 `.context/system/` 事实。

## 退出标准

- `cargo test` 覆盖单实例获取、二次启动转发激活、命令分发与优雅退出。
- `cargo check` 与 `ctx validate` 通过。
- 文档明确：GUI/托盘尚未实现。

## 检查点

- 模块划分符合设计依赖方向（core 不被 app 之外反向污染）。
- 用户审查前不提交、不推送本阶段代码变更（除非用户另行授权）。

## 计划级风险

- 过早引入 GUI 依赖导致版本锁定失败：本计划刻意推迟 GPUI。
- 单实例在真实 OS 上的锁语义与内存 fake 不同：后续任务再加平台适配，本阶段只锁定业务协议。

## 完成标准

- 仓库具备可扩展 workspace 骨架与可测运行时协议。
- 开放风险与实现状态已写入 system 文档。
