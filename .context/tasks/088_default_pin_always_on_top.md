# 任务 088：贴图默认置顶设置

- 状态：已完成
- 计划：`.context/plans/088_default_pin_always_on_top.md`
- 规模：大
- 依赖：v3 `AppSettings`、`SettingsStore`、设置面板、`desktop_shell` 新贴图创建和 `window_policy`。
- 生产行为变更：用户可保存后续新贴图的默认普通/置顶层级；已经存在的贴图不受影响。

## 任务目标

把固定的“新贴图总是置顶”改为可版本化、可迁移、可回读验证的本地偏好，并将该偏好限制在成功保存后的新贴图创建点。

## 范围

- 将 `AppSettings`/`SettingsRepairs` 和固定长度设置 codec 从 v3 升级为 v4，兼容读取 v1/v2/v3。
- 在设置面板加入明确的默认置顶 ON/OFF 行，并调整固定布局以避免新增行与操作按钮重叠。
- 在 `desktop_shell` 的新贴图呈现路径使用成功保存的默认值；已有贴图只保持自身现有状态。
- 更新 `AGENTS.md`、088 计划/任务、`.context/system/{overview.md,risks.md}`。

## 预期文件

- `crates/pinora-core/src/{settings.rs,lib.rs}`
- `crates/pinora-app/src/{settings_store.rs,settings_panel.rs,settings_window.rs,desktop_shell.rs}`
- `AGENTS.md`
- `.context/plans/088_default_pin_always_on_top.md`
- `.context/tasks/088_default_pin_always_on_top.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不修改已打开贴图、不实现点击穿透或全局置顶策略、不新增平台 API、依赖、窗口、事件循环、线程、网络或权限。
- 不重做 tray、截图、贴图编辑、历史、OCR、导出、主题或任务栏/Dock/分页器策略。

## 验收标准

1. v4 设置可严格编码、原子保存、回读；v1/v2/v3 文件默认迁移为置顶且不丢失既有字段。v4 默认置顶字节仅接受 `0`/`1`，其他值严格拒绝、保留源文件并走既有无效设置降级。
2. 设置面板的 ON/OFF 控件和方向键编辑可切换草稿，取消恢复原值，行与按钮布局不重叠，浅深主题仍有帧差异。
3. 只有成功保存更新后续新贴图的层级请求；保存失败和已存在贴图不变。
4. 不新增窗口路径或后台资源；`window_policy`、tray-only 和现有业务测试不回归。

## 验证

- `cargo test -p pinora-core settings -- --nocapture`
- `cargo test -p pinora-app settings_store -- --nocapture`
- `cargo test -p pinora-app settings_panel -- --nocapture`
- `cargo test -p pinora-app desktop_shell -- --nocapture`
- `cargo test -p pinora-app window_policy -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：schema 迁移破坏旧设置。缓解：逐版本 decoder、默认置顶迁移、原子回读和 codec 测试。回滚：保留 v4 decoder，忽略该字段且使用旧固定置顶默认。
- 风险：保存失败提前影响窗口层级。缓解：仅在 `SettingsStore::save` 成功分支更新内存默认值。回滚：删除成功分支更新，恢复固定默认。
- 风险：平台不接受置顶请求或普通层级窗口仍被窗口管理器提升。缓解：不伪造平台能力，原生会话单独验收。回滚：恢复既有置顶请求。

## 完成记录

- 已将设置 schema 从 v3 升级至 v4，并为默认贴图置顶增加严格 `0`/`1` 编码、v1/v2/v3 默认迁移、原子回读和损坏源文件保留测试。
- 已在既有设置窗口实现 `OFF`/`ON` 控件、方向键编辑、保存失败草稿保留、取消恢复以及无重叠布局。
- 已将成功保存后的默认值限制于后续新贴图的领域状态和窗口层级请求；恢复关闭贴图保留快照层级，已有贴图不变。
- 验证：`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 246 通过、2 忽略；core 88 通过）、`cargo check --workspace --target x86_64-pc-windows-msvc`、`git diff --check`。真实窗口管理器、任务栏/Dock/分页器、焦点和 HiDPI 尚未验证。
