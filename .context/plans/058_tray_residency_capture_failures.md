# 计划 058：可恢复截图失败的 tray 常驻边界

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/058_tray_residency_capture_failures.md`

## 目标

让区域、全屏、指定显示器、延时和窗口截图的可恢复失败统一结束当前捕获会话并回到 tray 空闲态。错误不得因 worker 返回、断开、像素预处理不一致或 Overlay 创建失败而退出主事件循环，也不得保留遮罩、控制窗口或任务栏/Dock 新入口。

## 非目标

- 不将窗口绘制/事件循环的不可恢复内部错误伪装为可恢复捕获失败，不改造进程级退出、单实例或后台任务关闭协议。
- 不新增通知、诊断面板、错误历史、重试按钮或更改已有 tray 菜单。
- 不改变真实捕获后端、区域/显示器/窗口目标解析、持久化格式、状态字符串或权限语义。

## 依赖关系

- 复用 054 的 `window_policy`、056 的延时恢复、057 的窗口失败回 tray 和现有 `LoadingState`。
- 依赖后台捕获 worker 已只跨线程传递 `ErrorCode`，以避免恢复日志泄露后端窗口文本。

## 约束

- `poll_loading_to_overlay` 的捕获错误处理不得调用 `event_loop.exit()`；事件循环只因显式退出或既有不可恢复主循环错误结束。
- 任一失败必须清除 `loading`、重置 `Mode::Idle`、清空等待状态并恢复 `FrameCache`；延时会话还必须恢复它在开始时隐藏的贴图。
- 日志只能输出稳定错误码和受控操作名，不输出像素、窗口标题、内部窗口 ID 或后端原始错误文本。
- Overlay 已局部创建但后续初始化失败时必须正常释放窗口资源，并回到 tray；所有窗口创建继续经 `window_policy`。

## 检查点

- 普通截图 worker 错误、worker 断开、缓冲不一致和 Overlay 创建错误不再由 `about_to_wait` 退出应用。
- 窗口与延时截图保留其现有的专门清理语义，且不会错误恢复先前隐藏的贴图。
- 失败恢复后新的 tray、热键或 IPC 截图请求可继续进入现有工作流。

## 计划级风险

- 无 GUI 会话的测试不能证明每种合成器已销毁已创建但失败的窗口，必须继续记录真实桌面缺口。
- 把所有错误降级会掩盖真正的逻辑错误；本任务只处理 `LoadingState` 的捕获工作流，主循环渲染错误保持既有处理。
- 终端日志不是用户可见恢复 UI；后续诊断能力需另立任务，不能在此任务伪造交互反馈。

## 阶段

1. 将 `LoadingState` 的失败分支与事件循环退出路径分离，建立统一的 tray 恢复出口。
2. 为普通、延时和窗口目标补充离线状态/结构回归测试，确认窗口策略入口不改变。
3. 执行 workspace 门禁、`ctx validate` 与 GitHub Linux/macOS/Windows CI，记录真实桌面缺口。

## 变更前记录

```text
目的：保证任意可恢复截图失败不会杀死 tray 常驻进程。
影响路径：desktop_shell 截图加载状态机、上下文文档。
兼容性：不改变公共接口、持久化数据、状态字符串、租户或权限语义。
外部副作用：无；只改变本机事件循环内的失败恢复。
回滚点：恢复 LoadingState 错误向主循环返回的旧行为；不影响后端、数据或窗口策略。
验证场景：worker 失败/断开、缓冲不一致、Overlay 创建失败、延时贴图恢复、窗口失败、严格门禁和 CI。
```

## 完成标准

- 任意 `LoadingState` 捕获失败都保持 tray、热键和 IPC 主循环存活，且不遗留 loading/Overlay 或额外窗口。
- 延时和窗口分支继续执行各自的可见性/隐私恢复，不输出敏感后端文本。
- 定向测试、workspace 严格门禁、`ctx validate` 与 GitHub 三平台 CI 通过；真实任务栏/Dock/合成器缺口如实记录。

## 完成记录

- 已实现：`LoadingState` 失败恢复改为无错误返回；`about_to_wait` 不再因它调用 `event_loop.exit()`。普通、窗口和延时捕获通过显式恢复范围清理 loading、模式、等待和帧缓存；延时范围优先且在像素已取得后 Overlay 失败时仍保留其恢复语义。
- 已验证：`cargo fmt --check`、`cargo check --workspace`、严格 Clippy、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 155 通过、2 忽略；core 57 通过）、`git diff --check` 与 `ctx validate` 通过；提交 `36ee681` 的 GitHub CI `30737231248` 已在 Linux/macOS/Windows 通过。
- 未覆盖：真实桌面上的 tray 连续驻留、窗口销毁、任务栏/Dock 与合成器可见性仍需原生会话。
