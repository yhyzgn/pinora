# 任务 102：KDE 真实显示器探测

- 状态：已完成
- 计划：`.context/plans/102_kde_real_display_probe.md`
- 规模：中
- 依赖：`KdeSpectacleCaptureProvider`、`DisplayInfo`、`kscreen-doctor`/`xrandr` 输出格式。
- 生产行为变更：是；移除固定分辨率伪探测，避免错误截图坐标。

## 任务目标

将 KDE 后端的显示器探测兜底改为真实执行和解析 `xrandr --query`；系统无法提供可信拓扑时返回受控不可用，不创建伪显示器。

## 范围

- `crates/pinora-app/src/capture_kde.rs`
- `AGENTS.md`
- `.context/plans/102_kde_real_display_probe.md`
- `.context/tasks/102_kde_real_display_probe.md`
- `.context/system/{overview,conventions,risks}.md`

## 非目标

- 不改变 Spectacle 捕获、截图会话、tray 菜单、IPC、窗口策略、设置 schema 或其他平台后端。
- 不新增依赖、fake 生产后端、屏幕拼接或隐式 Wayland 支持。

## 预期文件

- `crates/pinora-app/src/capture_kde.rs`：xrandr 探测、解析器与纯文本测试。
- `AGENTS.md`：切换当前工作指针。
- `.context/plans/102_kde_real_display_probe.md`、`.context/tasks/102_kde_real_display_probe.md`：计划和任务记录。
- `.context/system/{overview,conventions,risks}.md`：事实、验证命令和 R-061 风险记录。

## 验收标准

1. `kscreen-doctor` 失败时执行 `xrandr --query`，只返回已连接且有正尺寸 geometry 的输出。
2. 支持 primary 标记、负坐标和常见 `WIDTHxHEIGHT+X+Y`/`WIDTHxHEIGHT-X-Y` 形式。
3. xrandr 失败、无 connected 输出或 malformed geometry 返回 `CapabilityUnavailable`，不再返回固定 `3840x2160`。
4. 既有 Spectacle 捕获和单/多显示器快速路径行为不改变。

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

- 风险：不同驱动的 xrandr 文本格式可能无法完整解析；解析器宁可拒绝能力，也不能猜测分辨率。
- 回滚：禁用 KDE 后端或仅保留成功的 `kscreen-doctor` 路径；不恢复伪拓扑。

## 完成记录

- 已移除固定 `3840x2160` 虚拟显示器回退；`kscreen-doctor` 失败时执行真实 `xrandr --query`，严格解析 connected 输出、primary 标记、正负坐标和物理尺寸。
- xrandr 命令失败、无 connected 输出或 geometry 无效时返回 `CapabilityUnavailable`，不创建显示器条目、不生成错误截图资产；Spectacle、区域/全屏裁剪和其他平台路径保持不变。
- 已验证 `capture_kde` 8 项定向测试、workspace check、严格 Clippy、Windows target、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 302 通过、2 忽略；core 90 通过）、格式、`ctx validate` 与 `git diff --check`。真实 KDE/X11/Wayland 探测兼容性继续由 R-061 跟踪。
