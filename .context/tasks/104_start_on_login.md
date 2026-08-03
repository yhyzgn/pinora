# 任务 104：用户级开机自启

- 状态：已完成
- 计划：`.context/plans/104_start_on_login.md`
- 规模：中
- 依赖：设置 schema v8、原子 `SettingsStore`、tray-only `window_policy`、官方平台 autostart 规范。
- 生产行为变更：是；用户保存设置时会创建、更新或移除其当前账户内的 Pinora 启动项。

## 任务目标

为 tray-only Pinora 实现默认关闭、用户显式控制且可补偿回滚的用户级开机自启；登录启动不得自动截图，也不得让辅助窗口成为常驻任务栏、Dock 或分页器入口。

## 变更前记录

```text
目的：兑现开机自启设置，使登录后 Pinora 直接回到 tray-only 空闲生命周期。
影响路径：pinora-core 设置、settings_store、settings_panel、desktop_shell、平台启动项适配器、上下文文档。
兼容性：设置记录从 v8 升级到 v9；历史、贴图、截图、热键、IPC、权限和 tray-only 语义不变。
外部副作用：仅在用户明确保存开关时管理当前账户的 Pinora 启动项；不访问网络或共享服务。
回滚点：关闭开关入口和适配器调用，保持已存在 v9 配置可读；不删除未验证所有权的系统项。
验证场景：schema 迁移、v9 往返、托管项内容/所有权、启用/禁用补偿、面板命中、workspace 与跨 target 门禁。
```

## 范围

- `crates/pinora-core/src/settings.rs`
- `crates/pinora-app/src/{settings_store,settings_panel,settings_window,desktop_shell}.rs`
- 新增平台启动项适配器及其离线契约测试。
- `AGENTS.md`、`.context/plans/104_start_on_login.md`、`.context/tasks/104_start_on_login.md`、`.context/system/{overview,conventions,risks}.md`

## 非目标

- 自动更新、开机自动截图、系统服务、机器级注册、启动项外部编辑器或跨用户管理。
- 将真实登录、tray、Dock、任务栏、权限或启动性能用 CI/交叉编译替代。

## 预期文件

- `crates/pinora-core/src/settings.rs`：schema v9 与默认开关字段。
- `crates/pinora-app/src/{settings_store,settings_panel,desktop_shell,start_on_login}.rs`：设置编解码、面板交互、保存编排和平台启动项适配器。
- `src/main.rs`：自启动内部参数与已有实例不触发截图的转发规则。
- `crates/pinora-app/src/diagnostics_export.rs`：仅导出该设置布尔摘要。
- `AGENTS.md`、设计文档及 `.context/`：工作指针、规格、事实、验证和风险记录。

## 验收标准

1. 设置默认关闭，v1-v8 迁移稳定补齐默认值，v9 原子往返保持开关。
2. 平台适配器只创建/删除带 Pinora 所有权标记的用户级启动项，失败不会报告成功。
3. shell 只在用户保存开关变化时执行适配器；保存失败时恢复原平台注册状态与 runtime 设置。
4. 面板可键盘和鼠标切换该二元设置，不增加窗口或展示入口。

## 验证

- `cargo test -p pinora-core settings -- --nocapture`
- `cargo test -p pinora-app settings_store -- --nocapture`
- `cargo test -p pinora-app settings_panel -- --nocapture`
- `cargo test -p pinora-app start_on_login -- --nocapture`
- `cargo test -p pinora-app desktop_shell -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：真实桌面登录顺序、Windows Run/launchd 行为、受限用户目录和 tray 初始化仍需原生平台探针。
- 回滚：恢复 v8 迁移读取、隐藏设置入口并停用适配器调用；不删除非 Pinora 托管项。

## 完成记录

- 已完成 v9 设置记录、v1-v8 默认关闭迁移、设置面板键盘/鼠标开关、平台用户级启动项适配器和保存失败补偿。
- Linux `.desktop`、Windows Run、macOS LaunchAgent 均显式传递 `--pinora-autostart`；启动已有实例时不转发 `CAPTURE`，未知启动项不会覆盖或删除。
- 定向测试和完整质量门禁通过；真实登录会话、平台权限、tray/Dock/任务栏/分页器与启动性能未验证，已登记 R-063。
