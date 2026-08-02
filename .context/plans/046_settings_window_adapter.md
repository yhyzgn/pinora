# 计划 046：设置窗口适配器拆分

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/046_settings_window_adapter.md`

## 目标

沿用 045 的适配器边界，将设置窗口的 winit/softbuffer 生命周期、草稿面板、鼠标命中、原子保存句柄和呈现从 `desktop_shell` 提取为 `settings_window`，保持保存后策略应用仍由 shell 编排。

## 非目标

- 不增加设置字段、不改变 `settings.bin` codec、默认值或运行时策略。
- 不接入系统主题、原生控件、读屏或跨平台设置目录策略。
- 不将结构拆分作为真实 GUI/无障碍验收。

## 依赖关系

- 依赖 041 的版本化设置、原子保存与面板纯状态。
- 复用 045 的窗口适配器边界和既有 softbuffer 上下文。

## 约束

- 适配器只封装窗口、面板草稿与既有 `SettingsStore` 保存调用；不直接修改 runtime、历史索引或贴图策略。
- `desktop_shell` 必须继续在保存成功后才应用设置和历史配额清理。
- 不改变公开 IPC、持久化格式、状态字符串或用户权限语义。

## 检查点

- 保存失败仍保留草稿，runtime 和历史策略保持旧值。
- 关闭、Esc、resize、鼠标/键盘事件与 041 保持等价。
- 适配器绘制失败继续作为 `PinoraError` 上报给 shell。

## 计划级风险

- 保存与 UI 状态被拆到不同模块后可能导致提前应用草稿；接口只暴露保存结果，shell 在成功分支处理策略。
- 真实主题、焦点、HiDPI 和读屏缺口不因模块化而缩小，继续记录。

## 阶段

1. 新建 `settings_window` 并迁移窗口、面板、存储和 paint。
2. 将 desktop shell 改为调用最小适配器 API。
3. 运行回归门禁并更新事实/风险。

## 变更前记录

```text
目的：降低 desktop_shell 对设置窗口资源和草稿的耦合，建立可复用的 UI Adapter 模式。
影响路径：settings_window 新模块、desktop_shell、lib 模块声明、上下文文档。
兼容性：settings.bin、设置字段、热键、历史配额和运行时应用顺序不变。
外部副作用：仅写既有本地 settings.bin；不连接外部服务。
回滚点：移除适配器并回迁状态；设置存储格式与值不受影响。
验证场景：保存成功/失败、取消、鼠标/键盘动作、resize、历史配额落盘和清理回归。
```

## 完成标准

- shell 不再定义或直接绘制设置窗口状态。
- 保存成功才应用 runtime/历史策略的语义不变。
- workspace 质量门禁和 ctx validate 通过，真实 GUI 缺口明确保留。

## 完成记录

- 2026-08-02：新增 `settings_window` 适配器，承接设置窗口创建、焦点/关闭、草稿面板、原子保存、resize 与 softbuffer 呈现。
- 2026-08-02：`desktop_shell` 仅在适配器保存成功后应用 runtime 设置和历史配额清理；窗口创建失败时控制窗仍保持可见。
- 验证：设置面板 4/4、runtime 11/11、历史导出 13/13；workspace 112 app + 54 core 测试通过，2 个真实桌面测试忽略；fmt、check、严格 Clippy、diff 检查和 ctx validate 通过。
- 未覆盖：真实主题、焦点、HiDPI、读屏和平台设置路径验证。
