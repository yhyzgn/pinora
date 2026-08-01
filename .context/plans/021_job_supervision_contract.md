# 计划 021：任务监督基础契约

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/021_job_supervision_contract.md`

## 目标

在不触碰遗留图形壳和真实 OCR 进程实现的前提下，建立可离线验证的耗时任务监督基础：任务具有稳定 ID、关联 ID、资产版本、owner、取消令牌、截止时间与终态，并能在 owner 失效、显式取消或超时时拒绝结果提交。

## 非目标

- 不将新监督器接入 `desktop_shell`、OCR、导出或剪贴板路径。
- 不新增线程池、异步运行时或第三方依赖。
- 不用通用线程抽象假装可以强制终止任意闭包；外部子进程的强制回收留给 OCR 适配器切片。

## 约束

- 领域 ID、owner 和资产引用保持在 `pinora-core`；监督器只位于应用层。
- 取消、owner 失效和超时必须是可测试状态迁移，不能依赖睡眠竞争或真实外部进程。
- 已取消、超时或 owner 无效的任务结果只能被丢弃或报告为终态，不能被调用方接受。
- 不使用 warning suppression，不把窗口句柄、CLI 参数或线程句柄泄漏到领域模型。

## 依赖关系

- 依赖 019 提供的 `AssetRef` generation 契约。
- 依赖 020 的能力失败语义，任务层必须可承载可恢复错误。

## 阶段

1. 盘点既有 ID、资产引用和 OCR/剪贴板后台入口，确定最小领域类型与应用监督器边界。
2. 先为提交、取消、owner 失效、超时和匹配结果验收编写离线契约测试。
3. 实现纯内存 `JobSupervisor`，更新公共导出与上下文事实。

## 检查点

- 每个任务记录 `JobId`、`CorrelationId`、`AssetRef`、`JobOwner`、类型与截止时间。
- `JobSupervisor` 只接受仍在运行、owner 有效且资产版本完全匹配的结果。
- 取消与关闭 owner 会使后续提交不可接受，且可观察终态不依赖 UI。

## 计划级风险

- 若过早把此契约接入遗留壳，会扩大为 GUI 事件循环重写；本任务只交付可替换基础模块。
- 协作式取消本身不能终止阻塞的外部进程；后续 OCR 适配器必须持有并回收自己启动的 `Child`。

## 完成标准

- `pinora-core` 有可测试的 job/owner 值对象，应用层有可测试的内存监督器。
- 覆盖正常提交、显式取消、owner 关闭、超时与陈旧资产 generation 的拒绝场景。
- fmt、check、严格 Clippy、workspace 测试、差异检查和上下文校验通过。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：`pinora-core::job` 新增 `JobSpec`、`JobOwner`、`JobKind`、`JobResultRef` 与不可逆 `JobTerminalState`；`pinora-app::JobSupervisor` 新增协作式 `JobCancellation`、owner 关闭、显式取消、截止时间与结果提交门禁。
- 验证：领域任务元数据测试 1/1、监督器状态机测试 5/5 通过；`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`cargo test --workspace`（48 app + 39 core 通过，2 个真实桌面测试忽略）、`git diff --check` 和上下文校验通过。
- 残留风险：该模块尚未接入遗留 `desktop_shell` 或 OCR 子进程；它只能发出协作式取消，后续适配器必须在收到取消时回收自己创建的进程。
