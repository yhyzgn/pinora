# 任务 059：全虚拟桌面截图

- 状态：已完成
- 计划：`.context/plans/059_virtual_desktop_capture.md`
- 规模：中
- 依赖：`.context/tasks/054_auxiliary_window_boundary.md`、`.context/tasks/055_display_targeted_capture.md`、`.context/tasks/058_tray_residency_capture_failures.md`
- 生产行为变更：是；tray 新增“所有显示器截图”，KDE 可真实捕获单次工作区快照，其他无法提供一致快照的后端受控拒绝。

## 任务目标

新增“所有显示器”捕获意图和工作区来源元数据，将其从 tray 到 Overlay 的状态机接通。严格保持单屏、区域、窗口和延时路径行为，且所有失败返回 tray 空闲态。

## 范围

- 为核心捕获请求提供 `AllDisplays` 及安全的物理工作区 bounds 解析；为结果定义显式虚拟桌面 ID 和缩放语义。
- KDE 后端以单次 `spectacle -f` 工作区 PNG 实现该请求并验证严格尺寸；xcap 明确拒绝此请求。
- 在 tray、`desktop_shell` 与 Overlay 初始选择中接入该模式，并补齐离线回归测试。

## 非目标

- 不对多屏 xcap 做逐屏拼接、不新增 fake 回退，不实现跨屏连续区域选择、显示器默认值设置、真实 GUI 自动化或平台能力提示面板。
- 不添加任意窗口创建路径或改变已有窗口任务栏/Dock 策略。

## 预期文件

- `crates/pinora-core/src/{capture.rs,image.rs}`
- `crates/pinora-app/src/{capture_fake.rs,capture_kde.rs,capture_select.rs,capture_xcap.rs,desktop_shell.rs,tray.rs}`
- `AGENTS.md`
- `.context/plans/059_virtual_desktop_capture.md`
- `.context/tasks/059_virtual_desktop_capture.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. `AllDisplays` 不可被解析为单个显示器，工作区 bounds 对负坐标、间隔和溢出安全处理。
2. KDE 只接受尺寸严格匹配开始时工作区 bounds 的单次 `-f` PNG，并标记为虚拟桌面来源；不匹配不创建资产。
3. xcap 多显示器请求返回稳定的 `CapabilityUnavailable`，不得生成多时刻拼接图像；单屏既有请求不回归。
4. tray 操作进入全图 Overlay，失败不退出事件循环或遗留 Overlay；所有成功窗口继续仅走 `window_policy`。
5. 定向测试、fmt、workspace check、严格 Clippy、全量测试、diff 检查、`ctx validate` 与 GitHub 三平台 CI 通过。

## 验证

- `cargo test -p pinora-core capture -- --nocapture`
- `cargo test -p pinora-app capture_kde::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app tray::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：KScreen 拓扑和 `spectacle -f` 输出在热插拔时不同步。缓解：以开始快照的外接矩形严格匹配，不匹配即失败并回 tray。
- 风险：xcap 逐屏拼接会暴露跨屏时间不一致。缓解：明确拒绝，不以部分成功替代工作区成功。
- 风险：虚拟桌面没有单一 UI 缩放因子。缓解：来源 ID 明确标记，图像坐标固定为物理像素、scale 为 `1.0`。
- 风险：真实任务栏/Dock 与 KWin 像素行为未被离线门禁覆盖。缓解：沿用唯一窗口工厂并在完成记录中保留原生多屏桌面验证缺口。
- 回滚：移除 `AllDisplays` 请求、tray 项和相应分支；既有捕获接口、数据与窗口策略不需要迁移。

## 完成记录

- 已实现：`AllDisplays` 被建模为独立捕获意图，工作区外接矩形支持负坐标、显示器间隔并拒绝超出 `PixelRect` 坐标范围的异常拓扑。结果使用 `pinora:virtual-desktop` 来源和 `1.0` 物理像素缩放，不冒充任一显示器来源。
- 已实现：KDE 以单次 `spectacle -f` 取像，输出尺寸与开始时拓扑外接矩形不匹配时返回 `RetryablePlatform`；xcap 直接返回 `CapabilityUnavailable`，不进行逐屏拼接。tray 的多显示器菜单新增“所有显示器截图”，该路径不消费单屏缓存，成功后以 `window_policy` 创建的无边框虚拟桌面 Overlay 打开全图选区。
- 已验证：`cargo test -p pinora-core capture -- --nocapture`（9 通过）、KDE/xcap/tray/Overlay 定向测试通过；`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 159 通过、2 忽略；core 61 通过）、`git diff --check` 和 `ctx validate` 均通过。提交 `085e615` 的 GitHub CI `30737751448` 已在 Linux/macOS/Windows 通过。
- 未覆盖：没有真实多显示器 KDE 会话，故 Spectacle/KScreen 快照同步、外接矩形中空洞像素、Overlay 位置、任务栏/Dock、tray 交互、HiDPI、输入延迟和其他合成器行为尚未验证；三平台 CI 仅覆盖静态质量门禁。
