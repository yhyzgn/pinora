# 任务 055：托盘逐显示器全屏捕获与目标绑定缓存

- 状态：已完成
- 计划：`.context/plans/055_display_targeted_capture.md`
- 规模：中
- 依赖：`.context/tasks/048_full_display_capture.md`、`.context/tasks/047_frame_cache_handoff.md`、`.context/tasks/054_auxiliary_window_boundary.md`
- 生产行为变更：是；托盘新增逐显示器全屏截图入口，目标显示器绑定到捕获与缓存选择。

## 范围

- 让 `AppTray` 以当前 `DisplayInfo` 列表构造逐显示器全屏截图菜单，并将菜单事件解析为 `DisplayId`。
- 在 desktop shell 处理带目标的托盘动作，核验目标拓扑并只请求对应的 `CaptureRequest::FullDisplay`。
- 为 `FrameCache` 增加按显示器物理拓扑精确匹配的读取接口，覆盖命中与拒绝场景。
- 补充纯逻辑/托盘/缓存测试，并更新经过验证的上下文和风险。

## 任务目标

消除“多显示器环境按全屏截图却拍到默认大屏”的错误路径，使托盘提供可理解的显示器目标选择，同时保持默认快捷截图的现有低延迟路径。

## 非目标

- 不实现所有显示器拼接、跨屏 Overlay、自动刷新菜单、显示器设置面板、热插拔订阅或默认显示器持久化。
- 不改变截图后标注、复制、贴图、保存、历史或窗口生命周期。
- 不承诺指定非默认显示器一定命中预截缓存或在真实双屏环境达到固定时延。

## 预期文件

- `crates/pinora-app/src/{tray.rs,desktop_shell.rs,frame_cache.rs}`
- `AGENTS.md`
- `.context/plans/055_display_targeted_capture.md`
- `.context/tasks/055_display_targeted_capture.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 多显示器菜单项与 `DisplayId` 一一映射；无列表时保留既有默认动作。
2. 指定目标在捕获前重新验证，消失或变化时不改拍默认屏幕。
3. 缓存只在 ID、origin、尺寸和 scale 匹配时交付给目标；不匹配帧继续保留或由正确路径处理。
4. 默认 F2/F3、全屏初始选区、帧缓存暂停/恢复、托盘唯一常驻和辅助窗口任务栏隔离不回退。
5. 定向测试、fmt、workspace check、严格 Clippy、全量测试、diff 检查与 `ctx validate` 通过。

## 验证

- `cargo test -p pinora-app tray::tests -- --nocapture`
- `cargo test -p pinora-app frame_cache::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：菜单目标在热插拔后失效。缓解：捕获前重新枚举并精确比对目标 ID/拓扑；失败受控，不改变目标。
- 风险：错误消费缓存导致跨屏图像/坐标错配。缓解：匹配 ID、物理 origin、尺寸和 scale，并为拒绝场景补纯逻辑测试。
- 风险：菜单项过多或显示名称不稳定。缓解：显示后端提供的名称与物理分辨率，保持 `DisplayId` 仅内部保存。
- 回滚：删除带目标动作和匹配读取接口，恢复默认最大面积显示器捕获；不影响持久化数据与既有快捷键。

## 完成记录

- 2026-08-02：`AppTray` 现在以启动时的 `DisplayInfo` 列表创建多显示器全屏截图项目，并由菜单 ID 一对一解析为 `TrayAction::CaptureDisplay(DisplayId)`；单显示器和枚举失败时保留既有默认截图项。
- `CaptureTarget` 将指定 `DisplayId` 贯穿 desktop shell。指定目标必须先重新枚举并精确存在，无法找到时返回 `NotFound` 并保留 tray 常驻；`FrameCache` 只在 ID、origin、source rect 尺寸和 scale 全部一致时交付帧，四种拓扑错配均由离线测试拒绝。
- 本地通过 tray/frame cache/desktop shell 定向测试、fmt、workspace check、严格 Clippy、全量 workspace 测试（app 144 通过、2 个真实桌面测试忽略；core 55 通过）、diff 检查与 `ctx validate`。提交 `dfa8339` 的 GitHub CI `30735354166` 已在 Linux、macOS、Windows 原生 runner 通过格式、workspace 编译、严格 Clippy 和单元测试；真实双屏、热插拔、HiDPI、不同托盘实现和辅助窗口行为未覆盖。
