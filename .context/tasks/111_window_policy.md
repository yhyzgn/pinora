# 任务 111：桌面窗口策略边界

- 状态：已完成
- 计划：`.context/plans/111_window_policy.md`
- 规模：中
- 依赖：任务 050/061/066 的 tray-only 与 `window_policy` 契约、任务 109 桌面 crate。
- 生产行为变更：否；内部 crate 所有权迁移。

## 变更前记录

```text
目的：把辅助窗口的隐藏创建、平台任务栏/Dock 隔离和 KDE Wayland 映射后策略收敛到 pinora-desktop。
影响路径：workspace、desktop crate、app 的 desktop_shell/history/settings/diagnostics 窗口导入、上下文文档。
兼容性：窗口种类、标题、显示顺序、KWin 脚本、状态和失败降级均不改变。
外部副作用：仍仅在用户打开辅助窗口时创建窗口；Linux KDE 仍按既有脚本尝试，无新增网络或服务。
回滚点：恢复 app 内 window_policy/kwin_place 和直接导入，移除新导出。
验证场景：所有辅助窗口隐藏创建、显示入口、DisplayHandle 禁止映射、源码唯一建窗守卫、KWin 脚本稳定性和 Windows target。
```

## 任务目标

建立 `pinora-desktop` 的窗口策略边界，让 app 只负责调用而不拥有窗口创建与隔离实现。

## 范围

- 迁移 `crates/pinora-app/src/{window_policy,kwin_place}.rs` 至 `crates/pinora-desktop/src/`。
- 更新 desktop crate manifest/lib、app manifest/lib、`desktop_shell.rs`、history/settings/diagnostics window 导入。
- 更新设计文档、`.context/system/{overview,conventions,risks}.md`。

## 非目标

- 不迁移 EventLoop、托盘、Overlay/Pin 状态、截图、OCR、导出或历史业务。

## 预期文件

- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-desktop/Cargo.toml`、`crates/pinora-desktop/src/{lib,window_policy,kwin_place}.rs`
- `crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/{lib,desktop_shell,history_window,settings_window,diagnostics_window}.rs`
- `AGENTS.md`、`.context/{plans,tasks}/111_window_policy.md`
- `docs/Pinora-开发设计文档.md`、`.context/system/{overview,conventions,risks}.md`

## 验收标准

1. desktop crate 唯一拥有窗口策略/KWin 实现，app 删除旧模块并切换导入。
2. 既有 `window_policy` 与 KWin 测试、源码守卫、workspace/Clippy/Windows target/fmt/diff/ctx 门禁通过。
3. 不把静态检查、CI 或 fake 描述为真实任务栏/Dock/分页器验收。

## 验证

- `cargo test -p pinora-desktop -- --nocapture`
- `cargo test -p pinora-app --lib window_policy -- --nocapture`
- `cargo test -p pinora-app --lib kwin_place -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：跨 crate 可见性、winit target feature 或 `crate::kwin_place` 路径遗漏；窗口管理器实际忽略隔离策略。
- 回滚：恢复 app 内模块和导入，移除 desktop 导出；不删除用户文件和窗口状态。

## 完成记录

- `pinora-desktop` 已成为辅助窗口隐藏创建、显示、任务栏/Dock 隔离和 KDE KWin 位置策略的唯一实现；app 不再声明或编译旧 `window_policy.rs`、`kwin_place.rs`。
- `desktop_shell`、`history_window`、`settings_window`、`diagnostics_window` 均通过新 crate 调用；唯一 EventLoop 仍由 app 持有，DisplayHandle 显示守卫和源代码建窗守卫保持通过。
- 验证通过：desktop 定向 33 项、workspace 全量离线测试、workspace check、严格 Clippy、Windows target check、fmt、diff check 和 `ctx validate`。
- 已知缺口：真实窗口管理器是否最终隐藏任务栏/Dock/分页器、KWin 映射后焦点/首帧/HiDPI/性能仍未在本地验证。
