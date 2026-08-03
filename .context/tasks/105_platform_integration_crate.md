# 任务 105：系统集成功能 crate

- 状态：已完成
- 计划：`.context/plans/105_functional_crate_boundaries.md`
- 规模：中
- 依赖：任务 104 已提交的设置 v9 与用户级启动项；现有单实例、热键/Portal 实现；Rust workspace target 条件依赖。
- 生产行为变更：否；这是内部 crate 所有权迁移，用户行为和平台注册内容必须保持不变。

## 变更前记录

```text
目的：把启动项、单实例 IPC、全局热键和 Wayland Portal 从 pinora-app 拆入按功能命名的 pinora-platform。
影响路径：Cargo workspace、根入口、pinora-app 的 desktop_shell/runtime、迁移模块、上下文与设计文档。
兼容性：接口 / 数据 / 状态 / 租户 / 权限均不改变；保持设置 v9、IPC 帧和 tray-only 行为。
外部副作用：无新增平台注册、网络或共享基础设施访问；只有用户此前已保存的启动项在之后仍由同一适配器管理。
回滚点：恢复 pinora-app 的模块文件和依赖，移除 pinora-platform 引用。
验证场景：模块所有权、crate 依赖方向、定向单元测试、workspace/Windows target、严格 Clippy、全量离线测试和上下文校验。
```

## 范围

- 新增 `crates/pinora-platform/`，迁移 `start_on_login`、`single_instance`、`os_instance`、`hotkey` 与 Linux `wayland_portal`。
- 更新 `Cargo.toml`、`crates/pinora-app/Cargo.toml`、根 `src/main.rs`、`pinora-app` 的 runtime 与桌面壳导入。
- 更新 `AGENTS.md`、本计划/任务及 `.context/system/{overview,conventions,risks}.md`。

## 非目标

- 不拆分 `desktop_shell`，不迁移捕获、OCR、导出、历史、设置存储、窗口策略或 tray。
- 不改变实际用户启动项、注册表值、LaunchAgent 内容、热键默认值、IPC 命令、窗口创建策略或登录行为。

## 任务目标

建立并验证 `pinora-platform` crate，使启动项、单实例/IPC、全局热键和 Linux Wayland Portal 按系统集成功能集中归属；应用层只依赖其稳定公共契约。

## 预期文件

- `Cargo.toml`、`Cargo.lock`：注册 workspace 成员和共享依赖。
- `crates/pinora-platform/Cargo.toml`、`crates/pinora-platform/src/*.rs`：平台集成功能及原有契约测试。
- `crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/{lib,runtime,desktop_shell}.rs`：移除旧模块和直接平台依赖。
- `src/main.rs`：从平台 crate 组装单实例、IPC 和 desktop entry API。
- `AGENTS.md`、`.context/{plans,tasks}/105_*`、`.context/system/{overview,conventions,risks}.md`、设计文档：同步边界、证据和风险。

## 验收标准

1. `pinora-platform` 对 `pinora-core` 单向依赖；不得依赖 `pinora-app`、桌面壳、设置存储或业务 runtime。
2. `pinora-app` 与根入口使用平台 crate 的公开 API；原模块不存在或不再编译。
3. `cargo tree -p pinora-app` 不包含 `fs2`、`global-hotkey`、`zbus`、`async-channel`、`futures-lite` 的直接依赖声明。
4. 全部迁移模块的单元测试、workspace 严格门禁和 Windows target 通过；真实 GUI、登录会话和性能缺口继续如实保留。

## 验证

- `cargo test -p pinora-platform -- --nocapture`
- `cargo test -p pinora-app runtime -- --nocapture`
- `cargo test -p pinora-app desktop_shell -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：target 条件依赖遗漏会导致 Windows/macOS 编译失败；热键必须保留 GUI 线程生命周期，Portal worker 不能进入 UI 主线程。
- 回滚：将文件和依赖恢复至 `pinora-app`；不改写任何设置、启动项、IPC 文件或用户数据。

## 完成记录

- 已新增 `pinora-platform` 并迁移 `start_on_login`、`single_instance`、`os_instance`、`hotkey` 和 `wayland_portal`；`pinora-app` 和根入口已改用平台 crate API。
- 已移除 `pinora-app` 对 `fs2`、`global-hotkey`、`zbus`、`async-channel`、`futures-lite` 的直接声明，保留 target 条件和原有失败语义。
- 已通过平台定向测试、runtime/desktop shell 定向测试、workspace 全量测试、fmt、workspace check、严格 Clippy、Windows target、`git diff --check` 与 `ctx validate`。
- 已知缺口：真实桌面 GUI、登录会话、窗口管理器任务栏/Dock/分页器隔离和性能不属于本任务验证范围。
