# 计划 045：历史窗口适配器拆分

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/045_history_window_adapter.md`

## 目标

把历史窗口的 winit/softbuffer 资源、尺寸处理、预览缓存与 XRGB 呈现从 `desktop_shell` 提取为专属适配器，令桌面壳仅负责用户动作、历史事务和 Pin 编排。

## 非目标

- 不改变历史搜索、删除、清空、复用、PNG 校验或窗口视觉行为。
- 不迁移设置/Overlay/贴图窗口，也不新增 UI 框架、依赖或平台能力。
- 不将拆分宣称为真实 GUI/HiDPI/无障碍验证。

## 依赖关系

- 依赖 042/043 的 `history_browser` 纯状态和历史文件安全边界。
- 依赖 041 抽出的共享自绘原语与既有 softbuffer 上下文模式。

## 约束

- 新模块只拥有窗口适配和渲染缓存，不读取文件、不保存索引、不创建 Pin。
- `desktop_shell` 保持历史动作、受管文件操作和任务 owner 生命周期的唯一编排点。
- 所有导出符号先搜索引用；不改变 public IPC、历史 codec 或状态字符串。

## 检查点

- 关闭/重开、选择、窗口 resize、预览失效和 redraw 行为保持原语义。
- 适配器初始化和绘制错误仍转换为既有 `PinoraError` 并由 shell 处理。
- 纯状态和 workspace 回归测试在拆分后保持通过。

## 计划级风险

- winit `Rc<Window>` 与 softbuffer 生命周期拆分时容易出现借用或窗口关闭后访问错误；接口只暴露必要动作并锁定编译/测试门禁。
- 该任务降低文件体积但不解决真实桌面适配质量，风险记录不得关闭。

## 阶段

1. 建立内部 `history_window` 模块并迁移资源状态与绘制。
2. 将 shell 改为调用适配器，保留动作和事务逻辑。
3. 运行回归门禁、更新上下文与风险。

## 变更前记录

```text
目的：减少 desktop_shell 的窗口资源与渲染职责，建立可继续迁移的 UI Adapter 边界。
影响路径：history_window 新模块、desktop_shell、lib 模块声明、上下文文档。
兼容性：不改持久化、公共 IPC、状态字符串、权限或租户语义。
外部副作用：无；仍只在既有本地窗口和受管历史目录中运行。
回滚点：移除适配器并将调用点回迁，不影响 history.bin 与历史文件。
验证场景：历史窗口打开/关闭、选择和预览、resize、绘制失败传播、搜索/删除/清空回归。
```

## 完成标准

- `desktop_shell` 不再直接定义历史窗口状态或执行历史窗口绘制。
- 新适配器保持现有输入可见状态和错误传播。
- workspace 质量门禁和 ctx validate 通过；真实 GUI 缺口明确保留。

## 完成记录

- 2026-08-02：新增 `history_window` 适配器，承接历史窗口创建、焦点/关闭、尺寸同步、预览缓存和 softbuffer XRGB 呈现。
- 2026-08-02：`desktop_shell` 只通过适配器访问历史面板与 redraw，仍独占文件校验、清空/删除事务和 Pin 创建编排。
- 验证：历史浏览 4/4、历史导出 13/13；workspace 112 app + 54 core 测试通过，2 个真实桌面测试忽略；fmt、check、严格 Clippy、diff 检查和 ctx validate 通过。
- 未覆盖：真实窗口生命周期、HiDPI、焦点、读屏以及平台适配器验收。
