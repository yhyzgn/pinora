# 任务 101：KDE 指定显示器全屏捕获正确性

- 状态：已完成
- 计划：`.context/plans/101_kde_targeted_display_capture.md`
- 规模：小
- 依赖：`KdeSpectacleCaptureProvider`、`CaptureRequest`、显示器拓扑快照。
- 生产行为变更：是；多显示器指定全屏不再误用当前显示器快照。

## 任务目标

让 KDE 后端只在唯一显示器拓扑下使用 `spectacle -m`；多显示器指定显示器全屏必须使用一次 `-f` 全桌面快照并按目标 bounds 裁剪。

## 范围

- `crates/pinora-app/src/capture_kde.rs`
- `AGENTS.md`
- `.context/plans/101_kde_targeted_display_capture.md`
- `.context/tasks/101_kde_targeted_display_capture.md`
- `.context/system/{overview,conventions,risks}.md`

## 非目标

- 不改变区域/所有显示器/窗口捕获、截图后动作、热键、tray、设置或持久化格式。
- 不新增 Spectacle 参数假设、D-Bus 依赖、窗口、线程或 fake 生产回退。

## 预期文件

- `crates/pinora-app/src/capture_kde.rs`：快速路径判定与契约测试。
- `AGENTS.md`：切换当前工作指针。
- `.context/plans/101_kde_targeted_display_capture.md`、`.context/tasks/101_kde_targeted_display_capture.md`：计划和任务记录。
- `.context/system/{overview,conventions,risks}.md`：事实、验证命令和 R-060 风险记录。

## 验收标准

1. 拓扑仅有一个显示器且请求为该显示器 `FullDisplay` 时才允许 `-m`。
2. 拓扑包含多个显示器时，任何指定显示器 `FullDisplay` 都走全桌面快照和目标矩形裁剪。
3. 区域、`AllDisplays` 和窗口请求路径不改变；不存在错误显示器回退或 fake 成功。

## 验证

- `cargo test -p pinora-app capture_kde -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`
- `git diff --check`

## 风险与回滚

- 风险：多显示器全桌面捕获成本高于 `-m`；Spectacle/KWin 版本可能对全桌面边界或缩放返回不同尺寸，仍须原生 KDE 多显示器验证。
- 回滚：恢复旧快速路径条件或暂时隐藏指定显示器入口；不改变 tray/IPC、区域截图和错误语义。

## 完成记录

- 已将 `spectacle -m` 限制为唯一显示器的 `FullDisplay`；多显示器指定全屏统一使用单次 `-f` 全桌面 PNG，再按开始时拓扑和目标 bounds 裁剪。
- 已覆盖单屏保留快路径、多屏目标禁止快路径和全桌面尺寸校验；区域、AllDisplays、窗口截图及其错误语义不变。
- 已验证 `capture_kde` 6 项定向测试、workspace check、严格 Clippy、Windows target、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 300 通过、2 忽略；core 90 通过）、格式、`ctx validate` 与 `git diff --check`。真实 KDE 多显示器、异构缩放、性能和窗口管理器行为继续由 R-060 跟踪。
