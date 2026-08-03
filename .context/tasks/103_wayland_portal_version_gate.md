# 任务 103：Wayland Portal 版本门槛

- 状态：已完成
- 计划：`.context/plans/103_wayland_portal_version_gate.md`
- 规模：小
- 依赖：任务 100 `WaylandPortalHotkeys`、XDG `GlobalShortcuts` 版本属性。
- 生产行为变更：是；旧 Portal backend 明确降级而不误报成功。

## 任务目标

将 Portal 最低版本从 v1 收紧到当前实现所需的 v2，并以纯测试锁定版本不足时的受控状态。

## 范围

- `crates/pinora-app/src/wayland_portal.rs`
- `AGENTS.md`
- `.context/plans/103_wayland_portal_version_gate.md`
- `.context/tasks/103_wayland_portal_version_gate.md`
- `.context/system/{overview,conventions,risks}.md`

## 非目标

- 不改变 D-Bus worker 生命周期、shortcut ID、绑定选项、tray/IPC、设置或窗口策略。

## 预期文件

- `crates/pinora-app/src/wayland_portal.rs`：版本常量和契约测试。
- `AGENTS.md`：切换当前工作指针。
- `.context/plans/103_wayland_portal_version_gate.md`、`.context/tasks/103_wayland_portal_version_gate.md`：计划/任务记录。
- `.context/system/{overview,conventions,risks}.md`：事实、验证命令和风险记录。

## 验收标准

1. `PORTAL_MIN_VERSION` 为 2。
2. 版本不足映射 `VersionUnsupported`，且不创建 session/binding。
3. 现有 Portal 动作映射、后台线程和失败回退保持不变。

## 验证

- `cargo test -p pinora-app wayland_portal -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`
- `git diff --check`

## 风险与回滚

- 风险：backend 版本属性和实际方法实现可能不一致，方法调用失败仍需稳定降级。
- 回滚：仅恢复最低版本常量；不恢复 fake、同步 D-Bus 或 tray/IPC 改动。

## 完成记录

- 2026-08-03：已完成版本门槛实现、纯契约测试、workspace/Clippy/Windows target/格式/context 门禁；已提交并推送。
