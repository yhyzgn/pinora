# 任务 048：全屏截图用户入口与 Overlay 预选

- 状态：已完成
- 计划：`.context/plans/048_full_display_capture.md`
- 规模：中
- 依赖：`.context/tasks/047_frame_cache_handoff.md`、`.context/tasks/031_overlay_annotation_asset_contract.md`
- 生产行为变更：是；新增单显示器完整图像的 F3 与托盘入口，复用既有捕获、标注、导出和贴图路径。

## 范围

- 新增 `CaptureFullDisplay` 动作、F3 热键和托盘全屏截图菜单项。
- 在 `desktop_shell` 传递区域/全屏启动意图，使全屏帧在 Overlay 创建后自动建立完整有效选区。
- 为意图选择与完整选区边界增加离线回归测试。

## 任务目标

让用户无需拖动即可对当前捕获目标的完整图像执行标注、复制、保存、OCR 或贴图，同时不在热路径重新捕获或复制大像素缓冲。

## 非目标

- 不增加 all-displays 捕获、窗口捕获、延迟截图、显示器列表或 CLI/IPC 新命令。
- 不更改截图后端对“当前显示器”的平台定义。

## 预期文件

- `crates/pinora-core/src/action.rs`
- `crates/pinora-core/src/selection.rs`
- `crates/pinora-app/src/hotkey.rs`
- `crates/pinora-app/src/runtime.rs`
- `crates/pinora-app/src/tray.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `src/main.rs`
- `AGENTS.md`、`.context/plans/048_full_display_capture.md`、`.context/tasks/048_full_display_capture.md`
- `.context/system/overview.md`、`.context/system/risks.md`

## 验收标准

1. F3、全局热键事件和托盘菜单可请求全屏模式；F2/Ctrl+N 行为不变。
2. 缓存与 cold capture 都保持全屏启动意图，并以整张图像边界初始化有效 `Ready` 选区。
3. 区域、全屏和 Overlay 纯逻辑回归与 workspace 严格门禁通过。

## 验证

- `cargo test -p pinora-core action::tests -- --nocapture`
- `cargo test -p pinora-core selection::tests -- --nocapture`
- `cargo test -p pinora-app hotkey::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：全屏预选的末端像素计算错误，导致导出少一行或一列；通过 1×1、普通尺寸与偏移边界测试锁定。
- 风险：启动意图在 cold capture 线程完成后丢失；将意图放入加载状态，禁止从 UI 默认值重新推断。
- 回滚：删除 F3/托盘动作和预选初始化；区域截图与资产数据不变。

## 完成记录

- 2026-08-02：F3、托盘和 `CaptureFullDisplay` 动作接入完整图像模式；区域动作不变。
- 2026-08-02：`OverlayTarget` 使缓存与 cold capture 不会丢失启动意图；全屏时完整物理像素边界被自动确认，区域模式保持空选区等待拖动。
- 验证：`action::tests` 1/1、`selection::tests` 6/6、`hotkey::tests` 2/2、全屏 runtime 1/1、`desktop_shell::overlay_scale_tests` 10/10；workspace 117 app + 55 core 测试通过，2 个真实桌面测试忽略；fmt、check、严格 Clippy、diff 检查和 ctx validate 通过。
- 已知风险：F3 全局注册可被桌面环境拒绝，托盘和聚焦窗口入口仍可用；跨屏、HiDPI 和真实桌面交互尚未验证。
