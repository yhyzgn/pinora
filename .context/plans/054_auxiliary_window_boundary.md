# 计划 054：辅助窗口创建边界与托盘唯一常驻强化

- 状态：进行中
- 负责人：Codex
- 当前任务：`.context/tasks/054_auxiliary_window_boundary.md`

## 目标

将所有 Pinora 窗口构造收敛到 `window_policy`，确保 Overlay、贴图、历史、设置、兼容会话和隐藏 display-handle 在创建前均请求平台任务栏/Dock 隔离，并且可见窗口在映射后统一执行 KDE Wayland 的补充策略。空闲态继续只保留托盘、热键和 IPC，不创建主窗口。

## 非目标

- 不修改截图、标注、OCR、贴图、历史或设置的业务状态和交互。
- 不承诺标准 Wayland 或其他合成器可执行不存在的通用 skip-taskbar 协议。
- 不将静态测试或 CI 视为 Windows 任务栏、macOS Dock、KDE 或 HiDPI 的真实桌面验收。

## 依赖关系

- 依赖 050 已建立的 Windows/X11/macOS/KDE 策略及托盘失败即退出语义。
- 依赖当前 `winit`、`tray-icon` 和 KWin 适配器，不新增依赖或外部服务。

## 约束

- `create_window` 只能留在 `window_policy` 的辅助窗口工厂中；所有现有调用点迁移后不得直接绕过平台策略。
- 显示 Overlay、贴图和面板后必须通过策略模块触发 KWin 映射后隔离；隐藏 display-handle 不映射，仍须使用创建前属性策略。
- 保持 Windows `with_skip_taskbar(true)`、X11 Utility、macOS Accessory/`LSUIElement` 和 KWin `skipTaskbar`/`skipPager` 的既有语义。
- KWin 不存在或脚本失败不得阻塞截图、贴图、关闭或退出。

## 检查点

- 源码中所有生产窗口构造都由单一策略工厂执行。
- 所有可见窗口类型都有映射后策略调用；空闲态没有控制窗、主窗或启动自动弹窗。
- 既有 KWin 标题限定、临时脚本清理、托盘启动失败语义与历史/设置/兼容窗口行为保持不变。

## 计划级风险

- 该边界消除代码遗漏，不可替代 Windows、macOS、X11、KDE Wayland 的真实任务栏/Dock 检查。
- 其他 Wayland 合成器仍不保证接受应用侧任务栏隔离请求；必须保留已知限制。

## 阶段

1. 建立工厂与映射后策略 API，明确隐藏和可见窗口种类。
2. 迁移主 shell、历史、设置和兼容适配器的全部窗口构造点。
3. 用定向和 workspace 门禁验证结构与既有交互，记录 CI 和真实桌面缺口。

## 变更前记录

```text
目的：把“Pinora 只在 tray 常驻，临时窗口不进入任务栏/Dock”的策略从分散调用提升为单一创建边界。
影响路径：window_policy、desktop_shell、history_window、settings_window、region_overlay、pin_window、上下文文档。
兼容性：不改变公共接口、持久化数据、状态字符串、权限、租户、截图或贴图交互。
外部副作用：仅继续在 KDE 用户会话异步调用既有 KWin 脚本；失败不影响主流程。
回滚点：恢复各调用点的现有属性包装与 KWin 调用；不影响图像、历史或设置数据。
验证场景：各辅助窗口创建点均使用策略、KWin 脚本转义/生成、无控制窗状态、三平台编译测试、真实桌面缺口登记。
```

## 完成标准

- 所有生产窗口构造都通过 `window_policy` 单一工厂，且可见种类统一调用映射后策略。
- 空闲态仍只依赖托盘、已注册热键和 IPC；辅助窗口不会形成独立应用入口。
- 定向测试、workspace 严格门禁、`ctx validate` 和 GitHub 三平台 CI 通过；真实平台窗口验收缺口如实记录。

## 完成记录

- 2026-08-02：已将主 shell、历史、设置及兼容 Overlay/贴图会话的全部窗口创建迁移至 `window_policy::create_auxiliary_window`；生产源码中仅策略工厂保留直接 `create_window`。隐藏 display-handle 以独立类型受创建前策略约束，不触发映射后 KWin 操作。
- 可见 Overlay、贴图和面板均在可见后通过 `apply_post_map_policy` 触发既有的 KWin `skipTaskbar`/`skipPager` 请求；Windows `skip_taskbar`、X11 Utility、macOS Accessory/`LSUIElement` 的已有语义未改变。
- 本地通过窗口策略与 KWin 定向测试、fmt、workspace check、严格 Clippy、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 138 通过、2 个真实桌面测试忽略；core 55 通过）、diff 检查与 `ctx validate`。GitHub 三平台 CI 尚待本次提交后执行，任务保持进行中；真实 Windows/macOS/X11/KDE Wayland 任务栏、Dock 和分页器探针未运行。
