# 代码规范与上下文规则

## 当前工程约定

- Cargo workspace：唯一二进制入口为根 `src/main.rs`（package `pinora`）；领域在 `pinora-core`，系统集成在 `pinora-platform`，桌面编排/UI 仍在 `pinora-app`；后续 crate 按设计文档 `crates/pinora-*` 拆分。
- 依赖方向：`src/main.rs` 只负责组合 `pinora-app`、`pinora-platform` 和 `pinora-core`；功能 crate 只能向下依赖 `pinora-core`/能力端口，`pinora-core` 不得依赖 app、UI 或平台适配器。
- 命令表示意图、事件表示已发生事实；事件须带 `event_id`、`correlation_id`、`occurred_at_ms`；日志不得写入截图像素、OCR 全文或凭据。
- 平台能力通过 trait 注入：`CaptureProvider`、`ImageSink`、`SingleInstance`、`CapabilityProbe`、`HotkeySource`；测试用 fake/内存实现；入口用 `OsSingleInstance` + `LocalImageSink` + xcap/fake。
- 高层用户意图优先映射为 `ActionId`，再展开为 `Command`（`InvokeAction`）。
- **区域选区**几何在 `pinora-core::selection`；交互 Overlay 在 `pinora-app::region_overlay`（阻塞事件循环），不嵌入 `AppRuntime::dispatch`。
- 修改公共模块、类型或函数前，使用 `rg` 搜索全部引用，并运行 `cargo check --workspace` 与相关测试。
- 外部 CLI 适配器必须持有自己创建的 `Child`；超时先对句柄执行 `kill` 再 `wait`，不得通过 PID 字符串或外部 `kill` 命令回收，也不得把 `wait_with_output` 放进脱离调用方生命周期的线程。
- Pinora 空闲状态不创建控制窗口；托盘、已成功注册的全局热键与单实例 IPC 是后台入口。所有辅助 `WindowAttributes` 必须经 `window_policy::auxiliary_window_attributes`；新建事件循环必须使用 `window_policy::auxiliary_event_loop`，禁止绕开任务栏/Dock 策略。
- KWin 特例仅允许在窗口映射后按 Pinora 自身标题调用 `kwin_place`；`busctl` 失败只能记录，不能阻塞事件循环或被当作其他 Wayland 合成器的支持证据。

## 当前 crate 边界（任务 105 已验证）

```mermaid
graph LR
    Main["src/main.rs"] --> App["pinora-app\n桌面编排/UI"]
    Main --> Platform["pinora-platform\n系统集成"]
    App --> Platform
    App --> Core["pinora-core\n领域模型"]
    Platform --> Core
```

- `pinora-platform` 唯一拥有 `start_on_login`、`single_instance`、`os_instance`、`hotkey` 和 Linux `wayland_portal`。
- `pinora-app` 当前仍拥有截图选择器、任务监督、存储、导出、OCR、Overlay/贴图、托盘和窗口编排；这些边界按后续任务逐一拆分。

## 系统依赖（Linux + xcap）

```bash
sudo dnf install -y pipewire-devel mesa-libgbm-devel wayland-devel libxcb-devel
```

缺库时链接失败或运行期降级到 fake。

## 验证命令

- 工程元数据：`cargo metadata --no-deps --format-version 1`
- 格式：`cargo fmt --check`
- 编译：`cargo check --workspace`
- 静态检查：`cargo clippy --workspace --all-targets -- -D warnings`
- 测试：`cargo test --workspace`
- 真捕获（可选）：`cargo test -p pinora-app real_capture -- --ignored`
- 运行探针：`cargo run`
- 上下文完整性：`python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

100 Wayland Portal 增量已执行并通过：`cargo test -p pinora-app wayland_portal -- --nocapture`（4 通过）、`cargo test -p pinora-app hotkey -- --nocapture`（16 通过）、`cargo test -p pinora-app desktop_shell -- --nocapture`（39 通过）、`cargo fmt --check`、`cargo check --workspace`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 298 通过、2 忽略；core 90 通过）、`ctx validate` 与 `git diff --check` 均成功。Portal 当前开发机仅验证为缺失接口时的受控不可用状态；不证明真实 Wayland 授权、全局触发、tray-only、任务栏/Dock 或性能。

101 KDE 指定显示器捕获修复已通过：`cargo test -p pinora-app capture_kde -- --nocapture`（6 通过）、`cargo fmt --check`、`cargo check --workspace`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo clippy --workspace --all-targets -- -D warnings` 和 `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 300 通过、2 忽略；core 90 通过）。该验证证明多显示器时不会选择当前显示器 `-m` 快路径，不证明真实 KDE 多显示器、异构缩放、性能或窗口管理器行为。

102 KDE 真实显示器探测修复已通过：`cargo test -p pinora-app capture_kde -- --nocapture`（8 通过）、`cargo fmt --check`、`cargo check --workspace`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo clippy --workspace --all-targets -- -D warnings` 和 `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 302 通过、2 忽略；core 90 通过）。该门禁证明不再返回固定虚拟拓扑，不证明真实 KDE/X11/Wayland 驱动输出、权限、截图准确性或性能。

103 Wayland Portal 版本门槛增量已通过：`cargo test -p pinora-app wayland_portal -- --nocapture`（5 通过）、`cargo fmt --check`、`cargo check --workspace`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo clippy --workspace --all-targets -- -D warnings` 和 `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 303 通过、2 忽略；core 90 通过）。该门禁只证明 v1 及更低版本不会进入绑定，不证明 backend 方法完整性、授权 UI、全局触发、tray-only 或性能。

104 用户级开机自启增量已完成实现：`cargo test -p pinora-app start_on_login -- --nocapture`（3 通过）覆盖 Linux `.desktop` 参数转义、`--pinora-autostart` tray-only 参数、同目录同步原子写入和未知项所有权冲突；设置 schema v1-v8 迁移到 v9 并补充 v9 往返。新鲜完整门禁 `cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 307 通过、2 忽略；core 90 通过）、`cargo check --workspace --target x86_64-pc-windows-msvc`、`git diff --check` 与 `ctx validate` 均通过。该验证不证明真实登录会话、平台权限、tray/Dock/任务栏/分页器或启动性能；macOS 当前为 LaunchAgent 兼容路径，不是 `SMAppService` 受管实现。

105 系统集成功能 crate 已完成：`cargo test -p pinora-platform -- --nocapture`（21 通过），workspace 全量测试（app 286 通过、2 忽略；core 90 通过；platform 21 通过；根入口 1 通过）、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --check`、`git diff --check` 与 `ctx validate` 均通过。`pinora-platform` 现唯一拥有启动项、单实例/IPC、全局热键和 Linux Wayland Portal；真实桌面 GUI、登录会话、任务栏/Dock/分页器和性能仍未验证。

106 捕获功能 crate 已完成：`pinora-capture` 现唯一拥有 KDE/Spectacle、xcap、显式 fake、后端选择和 `FrameCache`；`cargo test -p pinora-capture -- --nocapture` 为 25 通过、1 个真实桌面测试忽略，`cargo tree -p pinora-app --depth 1` 已确认 app 不再直接依赖 xcap。完整 workspace 测试、严格 Clippy、Windows target、fmt、diff 和 `ctx validate` 作为任务 106 最终门禁；真实屏幕权限、HiDPI、桌面后端和性能仍未验证。

107 任务监督 crate 已完成：`pinora-jobs` 唯一拥有 `JobSupervisor`、`JobCancellation`、结果门禁和有界 worker 回收；基础 7 项测试及 OCR、导出、历史加载的定向回归通过。完整 workspace、Clippy、Windows target、fmt、diff 和 `ctx validate` 作为任务 107 最终门禁；协作式取消的真实子进程收敛仍不能仅由离线测试证明。

108 本地存储 crate 已完成：`pinora-storage` 唯一拥有 `SettingsStore`、`HistoryStore`、`HistoryLoad` 和 `ExportNameAllocator`；存储定向 28 项、历史清理 16 项、desktop shell 39 项回归通过。完整 workspace、Clippy、Windows target、fmt、diff 和 `ctx validate` 作为任务 108 最终门禁；断电、只读/网络文件系统、权限和 GUI 行为仍未验证。

109 桌面交互原语 crate 已完成：`pinora-desktop` 唯一拥有贴图几何、Overlay 工具栏布局/命中和已提交预览缓存；crate 仅依赖 `pinora-core`，定向 25 项测试通过。完整 workspace、Clippy、Windows target、fmt、diff 和 `ctx validate` 作为任务 109 最终门禁；真实窗口、tray、HiDPI、合成器和帧时间仍未验证。

096 历史保留期增量已执行并通过：`cargo test -p pinora-core settings -- --nocapture`、`cargo test -p pinora-core history -- --nocapture`、`cargo test -p pinora-app --lib settings_store::tests -- --nocapture`、`cargo test -p pinora-app --lib settings_panel::tests -- --nocapture`、`cargo test -p pinora-app --lib history_export::tests -- --nocapture`、`cargo test -p pinora-app --lib desktop_shell::overlay_scale_tests -- --nocapture`；完整门禁使用上方 workspace、Clippy、测试、Windows target、差异和 `ctx validate` 命令。完整测试未连接真实共享数据库、缓存、消息队列、对象存储或第三方服务；2 个真实桌面测试按既有约定忽略。

097 历史最大磁盘占用增量使用同一组定向测试，并额外覆盖 v7 到 v8 设置迁移、非法容量修复、容量下调后的最旧优先 tombstone 与受管 PNG 清理；完整门禁仍使用上方 workspace、Clippy、测试、Windows target、差异和 `ctx validate` 命令。测试不连接真实共享基础设施；2 个真实桌面测试按既有约定忽略。

098 发布链路已完成：`cargo run --quiet -- --version` 输出 `pinora 0.1.0`；当前 `main` CI `30783363209`、tag package/release `30783568639` 和 `runtime-verify` `30783727003` 均成功。Release `v0.1.0-preview.8` 已确认 `isPrerelease=true`，下载全部 11 个资产后按合并 `SHA256SUMS.txt` 执行 `sha256sum -c` 全部通过；Linux tarball 清单包含 `/usr/bin/pinora` 和 desktop entry，runtime 报告已回写 Release body。该门禁不证明真实桌面 GUI、tray、热键、权限、签名/公证或性能。

099 脱敏诊断包增量已通过：`cargo test -p pinora-app --lib -- --nocapture`（294 通过、2 忽略）、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 和 `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace` 均成功。报告字段为固定白名单，写入使用同目录临时文件、`sync_all`、原子 rename 和可读性校验；测试不证明真实托盘点击、跨平台目录权限或文件系统断电语义。

## 跨平台构建与打包

- Linux CI/打包依赖：`libasound2-dev libgbm-dev libgtk-3-dev libpipewire-0.3-dev libwayland-dev libx11-dev libxkbcommon-dev libxkbcommon-x11-dev pkg-config rpm`。
- Linux 打包：`PINORA_VERSION=... PINORA_PLATFORM=linux bash packaging/package-unix.sh`。
- macOS 打包：`PINORA_VERSION=... PINORA_PLATFORM=darwin bash packaging/package-unix.sh`（原生 `macos-14` runner）。
- Windows 打包：`$env:PINORA_VERSION='...'; ./packaging/package-windows.ps1`（原生 `windows-2022` runner，NSIS 可选）。
- GitHub Actions：先运行 `ci.yml`，再运行 `package.yml`；tag 成功后由 `runtime-verify.yml` 下载同一 run 的 artifacts 做安装/`--version`/卸载 smoke。
- 本阶段未提供本机 macOS GUI 验证；Windows target 仅在 Linux 主机完成编译检查，运行验证以 GitHub 原生 runner 为准。

## Git 身份（固定，勿再询问）

本仓库本地固定使用以下提交身份，代理在提交/改写历史时直接使用，不得改用其他账号，也不得再次向用户确认：

```text
user.name  = Neo
user.email = yhyzgn@gmail.com
```

配置位置：仓库 `.git/config`（`git config --local`）。若缺失，提交前自动写回上述值。

## 文档与变更规则

- 稳定事实写入 `system/`；阶段顺序写入 `plans/`；一个有边界、可验证、可回滚的动作写入 `tasks/`。
- 证据与不确定性分开记录；设计文档中的建议不能直接升级为实现事实。
- 所有面向人员的上下文文档使用中文；路径、命令和无法翻译的技术标识符除外。
- 目前没有持久化数据访问层；后续数据库或 ORM 变更必须遵守仓库级 SQL 红线并补充隔离测试。
