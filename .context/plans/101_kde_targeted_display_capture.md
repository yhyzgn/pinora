# 计划 101：KDE 指定显示器全屏捕获正确性

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/101_kde_targeted_display_capture.md`

## 目标

修复 KDE/Spectacle 后端在多显示器场景下把指定显示器全屏请求错误地交给“当前显示器”快速路径的问题，确保目标显示器请求从一次全桌面快照中按拓扑裁剪，避免截错屏。

## 非目标

- 不改变区域、所有显示器、窗口截图、PNG 解码、坐标模型、热键、tray 菜单或设置 schema。
- 不把逐显示器拼接当作全桌面原子捕获，也不新增外部依赖、窗口或后台线程。
- 不把离线测试当作真实 KDE 多显示器、HiDPI 或 Spectacle 版本兼容证据。

## 约束

- 只能使用一次全桌面捕获再裁剪，禁止逐显示器拼接或以另一显示器/ fake 像素冒充目标结果。
- 保持 `CaptureRequest`、`CaptureImage`、显示器 ID、错误码、tray/IPC 和窗口策略不变。
- 任何外部命令仍由既有捕获 worker 调用；本任务不新增线程、窗口、依赖或日志敏感字段。

## 依赖关系

- 依赖 `pinora-core::CaptureRequest` 的显示器目标语义和 `resolve_capture_rect`。
- 依赖 KDE `DisplayInfo` 拓扑快照、Spectacle `-m`/`-f` 现有路径和 `CaptureImage::crop_local`。

## 检查点

1. 单显示器 `FullDisplay` 保留 `-m` 快速路径。
2. 多显示器指定任一目标都拒绝 `-m`，并沿全桌面裁剪路径继续执行。
3. 区域、AllDisplays、窗口捕获与尺寸不匹配错误保持原有行为。

## 阶段

1. 将 `-m` 快速路径限制为拓扑中唯一显示器的 `FullDisplay` 请求。
2. 为多显示器指定目标添加纯决策契约测试，并完成 workspace 门禁。
3. 更新 system/task 风险记录，提交并推送。

## 完成标准

- 多显示器 `FullDisplay { display }` 永不依赖当前鼠标所在屏幕，使用全桌面快照及目标 bounds 裁剪。
- 单显示器仍可使用 `-m` 快速路径；其他请求行为保持不变。
- 定向测试、workspace 编译、严格 Clippy、全量测试、Windows target、`ctx validate` 和 `git diff --check` 通过。

## 风险与回滚

- 多显示器全桌面 PNG 可能比单屏 `-m` 更慢、更大；失败仍返回受控错误，不生成错误资产。
- 回滚为恢复旧条件仅影响性能，不恢复截错屏语义；若 Spectacle 版本无法稳定返回拓扑一致的全桌面图，应暂时禁用指定显示器入口并保留 tray/IPC。

## 计划级风险

- Spectacle/KWin 可能在全桌面捕获时返回与 `kscreen-doctor` 不同的物理尺寸或原点，导致指定显示器入口需要暂时禁用。
- 多屏全桌面 PNG 的解码和裁剪可能提高首次截图延迟；必须以 KDE 原生探针和帧时间数据决定是否进一步引入直接 D-Bus ScreenShot2 适配器。

## 完成记录

- 已将 `spectacle -m` 限制为唯一显示器的 `FullDisplay`；多显示器指定全屏统一使用单次 `-f` 全桌面 PNG，再按开始时拓扑和目标 bounds 裁剪。
- 已覆盖单屏保留快路径、多屏目标禁止快路径和全桌面尺寸校验；区域、AllDisplays、窗口截图及其错误语义不变。
- 已验证 `capture_kde` 6 项定向测试、workspace check、严格 Clippy、Windows target、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 300 通过、2 忽略；core 90 通过）、格式、`ctx validate` 与 `git diff --check`。真实 KDE 多显示器、异构缩放、性能和窗口管理器行为继续由 R-060 跟踪。
