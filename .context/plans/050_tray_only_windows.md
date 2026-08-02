# 计划 050：仅托盘常驻与辅助窗口任务栏隔离

- 状态：进行中
- 负责人：Codex
- 当前任务：`.context/tasks/050_tray_only_windows.md`

## 目标

将 Pinora 调整为仅通过系统托盘常驻。截图 Overlay、贴图、历史、设置和兼容路径的辅助窗口可在用户操作期间显示，但不得创建独立任务栏/Dock 应用入口；空闲状态不得保留控制窗口。

## 非目标

- 不实现新的截图、标注、OCR、历史或贴图业务能力。
- 不承诺所有 Wayland 合成器可接受应用自行请求“跳过任务栏”；标准 xdg-shell 没有对应通用协议。
- 不绕过桌面环境安全模型，不使用外部窗口枚举、PID 匹配或破坏性窗口规则。

## 依赖关系

- 复用现有 `winit` 0.30、tray 和 KWin D-Bus 脚本适配器，不新增依赖。
- 050 是 049 历史再次编辑的前置任务，确保其新窗口同样不出现在任务栏。

## 平台策略

| 平台/会话 | 策略 | 证据边界 |
| --- | --- | --- |
| Windows | `WindowAttributesExtWindows::with_skip_taskbar(true)` | CI 只验证编译与测试，待真实任务栏探针 |
| X11 | `_NET_WM_WINDOW_TYPE_UTILITY` | CI 只验证编译与测试，待窗口管理器探针 |
| macOS | `ActivationPolicy::Accessory` 与 App bundle `LSUIElement` | CI 只验证编译、打包和运行 smoke，待 Dock/菜单栏探针 |
| KDE Wayland | 现有 KWin 脚本按窗口标题设置 `skipTaskbar` | 脚本语义与离线测试，待真实 KDE 任务栏探针 |
| 其他 Wayland | 不伪造保证，记录 compositor 限制 | 保持能力缺口开放 |

## 约束

- 所有辅助窗口属性必须经一个内部窗口策略模块构造；禁止散落平台条件分支。
- 移除空闲常驻控制窗口，不以隐藏/极小窗口替代。
- KWin 适配器只操作自身唯一标题窗口，并保持现有 `busctl` 子进程失败可见、不影响主事件循环。
- 必须保留 Overlay、贴图、历史和设置的键盘/鼠标处理；去掉控制窗后，空闲触发依赖托盘、已注册全局热键与 IPC。

## 检查点

- `desktop_shell` 空闲状态无 `ControlState`、无控制窗创建或焦点抢占。
- Desktop shell 和旧 `region_overlay`/`pin_window` 的全部 `Window::default_attributes()` 辅助窗口均走统一属性策略。
- 仅在成功映射后对 KDE Wayland 调用窗口策略；KWin 不可用或脚本失败时不终止应用。
- macOS event loop 和打包 plist 均声明 accessory/agent 语义。

## 计划级风险

- Windows、X11、macOS 与 KDE Wayland 的代码路径只能通过原生桌面会话验证；CI 的编译、安装和版本探针不包含任务栏或 Dock 断言。
- 其他 Wayland 合成器没有通用 skip-taskbar 协议。移除控制窗后，在未注册全局热键的会话中只能由托盘或 IPC 启动截图。

## 阶段

1. 建立跨平台辅助窗口属性与 macOS event-loop 构造器，覆盖编译条件。
2. 将当前和兼容窗口路径迁移到统一策略，删除空闲控制窗。
3. 扩展 KWin 脚本，执行 KDE Wayland 的 `skipTaskbar` 设置，运行三平台 CI 与本地质量门禁。

## 变更前记录

```text
目的：确保 Pinora 仅通过托盘常驻，辅助窗口不污染任务栏/Dock。
影响路径：window_policy、desktop_shell、history_window、settings_window、region_overlay、pin_window、kwin_place、macOS 打包清单、上下文文档。
兼容性：不改变公共 IPC、历史/设置数据、状态字符串、权限或租户语义；移除空闲焦点控制窗，Wayland 无全局热键时以托盘/IPC 代替其本地快捷键兜底。
外部副作用：Linux/KDE 下调用既有用户会话 `busctl` KWin Scripting；失败仅记录，不影响截图或退出。
回滚点：移除统一属性策略并恢复控制窗；不会影响图像、索引、贴图或导出数据。
验证场景：各辅助窗口创建点均使用策略、KWin 脚本转义/生成、无控制窗状态、三平台编译测试、真实桌面缺口登记。
```

## 完成标准

- 代码路径不再创建常驻控制窗口，辅助窗口通过统一策略请求跳过任务栏/Dock。
- Windows/X11/macOS/KDE Wayland 有明确实现；其他 Wayland 不伪造保证。
- 定向测试、workspace 严格门禁和 GitHub 三平台 CI 通过，真实任务栏/Dock 验收缺口如实记录。

## 完成记录

- 2026-08-02：新增 `window_policy`，所有当前和兼容的 Overlay、贴图、历史、设置以及隐藏 display-handle 均经统一入口构造。
- `desktop_shell` 不再创建、显示、聚焦或处理空闲控制窗口；启动只保持托盘、已注册全局热键、IPC 和后台帧缓存，不会自动弹出截图 Overlay。
- Windows 请求 `with_skip_taskbar(true)`；X11 请求 Utility 类型；macOS 事件循环使用 `Accessory` 且 app bundle 写入 `LSUIElement`；KDE Wayland 在映射后按 Pinora 标题请求 `skipTaskbar`/`skipPager`。
- KWin 临时脚本在 load/run 失败时仍卸载并删除自身文件；本地定向与严格质量门禁已通过，待本提交的 GitHub CI。其他 Wayland 和真实任务栏/Dock/KWin 会话仍未验证。
