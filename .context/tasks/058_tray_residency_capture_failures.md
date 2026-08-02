# 任务 058：可恢复截图失败保持 tray 常驻

- 状态：已完成
- 计划：`.context/plans/058_tray_residency_capture_failures.md`
- 规模：中
- 依赖：`.context/tasks/054_auxiliary_window_boundary.md`、`.context/tasks/056_delayed_capture.md`、`.context/tasks/057_window_capture.md`
- 生产行为变更：是；后台捕获失败不再退出 Pinora 事件循环，而是回到 tray 空闲态。

## 任务目标

移除 `LoadingState` 到 `about_to_wait` 的可恢复错误退出链，让 tray 始终作为 Pinora 的唯一后台入口；失败后用户仍可从 tray、热键或 IPC 发起下一次截图。

## 范围

- 让 `poll_loading_to_overlay` 在 worker 错误、worker 断开、预览缓冲不一致和 Overlay 打开失败时执行受控恢复而不是向事件循环返回错误。
- 保留并统一窗口/延时/普通截图的 loading 清理、模式重置、帧缓存恢复和延时贴图恢复边界。
- 为纯失败策略和现有窗口策略补充回归测试，更新上下文的已验证事实和风险。

## 非目标

- 不改造所有 winit 绘制错误、不会添加 GUI 通知或诊断面板，也不会更改捕获后端、tray 菜单或文件格式。
- 不声称静态测试能验证真实任务栏、Dock、KDE Wayland 或其他合成器的窗口销毁与可见性。

## 预期文件

- `crates/pinora-app/src/desktop_shell.rs`
- `AGENTS.md`
- `.context/plans/058_tray_residency_capture_failures.md`
- `.context/tasks/058_tray_residency_capture_failures.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. `LoadingState` 的可恢复失败不会从 `about_to_wait` 调用 `event_loop.exit()`，tray 对象仍被持有。
2. 普通、窗口与延时路径分别清理 loading/等待/帧缓存；延时路径仍仅恢复它开始时隐藏的贴图。
3. 捕获失败日志只包含稳定错误码和受控上下文，不包含窗口标题、内部 ID 或后端原始文本。
4. 不增加窗口构造路径，所有成功 Overlay 继续经 `window_policy` 创建。
5. 定向测试、fmt、workspace check、严格 Clippy、全量测试、diff 检查、`ctx validate` 与 GitHub 三平台 CI 通过。

## 验证

- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：错误分类过宽会隐藏应用 bug。缓解：仅将 `LoadingState` 的预期捕获/预览/Overlay 建立失败恢复为 tray；渲染与退出逻辑不在本任务范围。
- 风险：Overlay 资源部分初始化失败。缓解：让局部 `Rc<Window>` 在错误返回时析构，并保持唯一 `window_policy` 工厂。
- 风险：延时截图恢复遗漏。缓解：继续经现有 `finish_delayed_capture_failure` 处理并由离线状态测试覆盖。
- 回滚：恢复 `poll_loading_to_overlay` 的 `Result` 错误传播；不触及捕获数据、历史、设置或窗口属性。

## 完成记录

- 已实现：以 `CaptureFailureScope` 固定普通、窗口或延时恢复范围；`poll_loading_to_overlay` 对 worker 错误、worker 断开、双渲染缓冲不一致和 Overlay 建立错误均在本地清理并返回。`about_to_wait` 仅轮询该方法，不能再接收其错误并退出事件循环。捕获失败日志只保留稳定错误码。
- 已验证：`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`（22 通过）、`cargo test -p pinora-app window_policy::tests -- --nocapture`（2 通过）、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 155 通过、2 忽略；core 57 通过）、`git diff --check` 和 `ctx validate` 通过。
- 已验证：提交 `36ee681` 的 GitHub CI `30737231248` 已在 Linux/macOS/Windows 通过。
- 未覆盖：真实 Windows/macOS/X11/KDE Wayland 中的捕获失败、tray、热键、IPC、Overlay 析构与任务栏/Dock 行为仍需原生会话；三平台 CI 不构成这些 GUI 行为的证据。
