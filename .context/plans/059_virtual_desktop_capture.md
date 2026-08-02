# 计划 059：全虚拟桌面截图

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/059_virtual_desktop_capture.md`

## 目标

交付“所有显示器截图”的完整、可审计路径：用户可从 tray 发起全虚拟桌面截图，真实后端必须返回一次工作区快照或明确拒绝，成功后进入全图编辑 Overlay；无论成功、取消或失败，Pinora 都只通过 tray 常驻，绝不留下任务栏或 Dock 入口。

## 非目标

- 不把 xcap 的多次逐显示器取像拼接为貌似原子的桌面快照。
- 不实现跨平台原生截图后端、跨屏连续区域 Overlay、默认目标设置或窗口候选高亮。
- 不改变历史数据格式、OCR、导出、贴图、全局热键或持久化配置。

## 依赖关系

- 复用 054 的 `window_policy`、055 的显示器目标捕获和 058 的任何失败回 tray 状态机。
- KDE `spectacle -f` 已提供单次全工作区 PNG；其显示器拓扑由 KScreen/xrandr 适配器提供。

## 约束

- `CaptureRequest::AllDisplays` 是独立意图，不得把任一 `FullDisplay` 目标或最大显示器误作全桌面。
- 工作区像素尺寸必须等于开始时显示器物理 bounds 的外接矩形；不匹配即为受控失败，禁止裁剪、缩放或猜测偏移。
- 多显示器工作区的 `CaptureMetadata.display` 必须使用明确的虚拟桌面 ID，缩放固定为 `1.0`，表示图像坐标已是物理像素且不继承任一显示器缩放。
- 不具备单次工作区快照能力的后端必须返回 `CapabilityUnavailable`；不得产生 fake 或多时刻拼接的成功资产。
- 新 Overlay、贴图或面板窗口仍只能经过 `window_policy::create_auxiliary_window`；失败回收沿用 058 的 tray 恢复出口。

## 检查点

- 核心层能计算安全的虚拟桌面外接矩形，并区分虚拟桌面与普通显示器来源。
- tray 提供全虚拟桌面操作；其目标、初始全选与错误恢复不会混淆单屏、窗口和区域截图。
- KDE 后端严格验证单次 `-f` 输出与快照拓扑，xcap 显式拒绝多屏工作区请求。

## 阶段

1. 增加受测试的工作区捕获契约与来源元数据，建立尺寸/拓扑失败边界。
2. 将 tray、桌面状态机和 Overlay 初始选择接入新意图，保持失败回 tray。
3. 对 KDE PNG 处理和 UI 纯状态添加回归测试，执行 workspace 门禁、ctx 校验和 GitHub 三平台 CI。

## 变更前记录

```text
目的：完成真实、可验证的所有显示器截图入口，而不牺牲截图一致性或 tray-only 窗口隔离。
影响路径：capture 核心契约、KDE/xcap 后端、tray 菜单、desktop_shell、上下文文档。
兼容性：新增内部捕获意图与明确的虚拟桌面来源 ID；不改变已有单屏/区域/窗口请求、历史持久化、状态字符串、租户或权限语义。
外部副作用：用户点击 tray 新菜单时，KDE 会调用一次本机 spectacle；其他后端受控失败，不访问共享或第三方服务。
回滚点：移除新 tray 操作与 `AllDisplays` 分支即可恢复原有单屏行为；无持久化迁移。
验证场景：工作区 bounds、虚拟来源元数据、KDE 尺寸验证、xcap 拒绝、tray 映射、Overlay 初始全选、失败回 tray、严格门禁与 CI。
```

## 完成标准

- 在 KDE 真实后端中，用户可从 tray 捕获一次工作区 PNG 并进入全图 Overlay，像素尺寸和物理虚拟桌面 bounds 严格一致。
- 在不能保证单次工作区快照的后端中，操作不创建图像、不打开 Overlay、不退出进程，回到 tray。
- 新路径没有额外窗口构造入口，且定向测试、workspace 严格门禁、`ctx validate` 与 GitHub 三平台 CI 均通过。

## 计划级风险

- 无真实多显示器 KDE 会话时，离线测试只能证明契约、菜单和失败分支，不能证明 KWin/Spectacle 的实际像素、延迟或任务栏/Dock 行为。
- `spectacle -f` 与 KScreen 拓扑在热插拔瞬间可能失配；严格拒绝优先于裁剪或错误对齐。
- 跨显示器物理 bounds 中的空洞区域没有可捕获像素；本任务采用外接矩形语义，实际合成器行为必须由真实桌面验收确认。

## 完成记录

- 已实现：新增 `CaptureRequest::AllDisplays`、安全工作区物理外接矩形和明确的虚拟桌面来源 ID。KDE 仅接受严格匹配开始拓扑的单次 `spectacle -f` PNG；xcap 明确返回 `CapabilityUnavailable`，不拼接多时刻显示器帧。
- 已实现：tray 在多显示器拓扑中提供“所有显示器截图”；桌面状态机拒绝使用单屏 `FrameCache`，成功时以无边框、指定物理位置的虚拟桌面 Overlay 呈现并继续经 `window_policy` 请求任务栏/Dock 隔离。该 Overlay 同时从 xcap 窗口候选中排除。
- 已验证：本地 fmt、workspace check、严格 Clippy、全量离线测试（app 159 通过、2 忽略；core 61 通过）、`git diff --check` 与 `ctx validate` 通过；提交 `085e615` 的 GitHub CI `30737751448` 已在 Linux/macOS/Windows 通过。
- 未覆盖：真实多显示器 KDE 会话的 Spectacle/KScreen 同步、像素空洞、Overlay 跨屏映射、任务栏/Dock、托盘事件、HiDPI 与输入延迟仍需原生桌面验证；CI 不构成这些 GUI 行为的证据。
