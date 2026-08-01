# 计划 038：跨平台构建、安装包与预发布交付

- 状态：进行中
- 负责人：Neo
- 当前任务：`.context/tasks/038_cross_platform_delivery.md`

## 目标

让 Pinora 在 GitHub Actions 的 Linux、macOS、Windows 原生 runner 上完成 workspace 编译，生成可校验的原始二进制和平台安装包，并通过 runner-safe 的安装、启动、卸载 smoke 后发布 `pre-release`。运行时能力必须诚实报告：Linux/KDE 专用捕获与托盘能力在其他平台不可用时只能降级，不得构造 fake 截图。

## 范围

- 将 GTK 依赖限制在 Linux，托盘初始化按平台编译。
- 为单实例和 CLI 转发提供跨平台 loopback IPC；Unix 保留现有 socket，Windows/macOS 使用 loopback TCP 端口文件。
- KDE capture、KWin placement、Linux CLI 剪贴板在非 Linux 平台安全降级。
- 增加版本探针、跨平台构建/打包脚本和 SHA-256 产物索引。
- 增加 CI、package、runtime-verify 三个 GitHub Actions workflow，支持手动调试和版本 tag 发布。
- 使用预发布 tag 进行真实 GitHub Actions 调试并创建 GitHub pre-release。

## 非目标

- 不把 runner 上的无头启动当作真实桌面交互、多显示器、HiDPI、系统剪贴板或权限验证。
- 不在本阶段重写 `desktop_shell` 的交互架构，不新增数据库对象或远程共享服务。
- 不声明尚未获得真实桌面证据的平台捕获能力已经等同 Linux/KDE。

## 约束

- 只使用原生 GitHub-hosted runner 和仓库已有 Rust 依赖；不引入共享基础设施。
- 每个平台保留 raw binary、安装包和 SHA-256 清单；包版本来自同一 Cargo 版本源。
- 平台降级必须返回能力不可用或明确说明，不生成 fake 像素。

## 依赖关系

- 依赖 Rust stable、GitHub Actions 原生 runner、Linux GTK/PipeWire 开发包和 Windows NSIS。
- runtime-verify 依赖 package workflow 成功上传的 artifact。

## 阶段

1. 建立 target-specific 依赖和跨平台 IPC。
2. 增加三平台打包脚本、校验和安装 smoke。
3. 增加 CI/package/runtime-verify，实际运行并修复 GitHub Actions。
4. 推送 preview tag，生成 prerelease 并回写运行时报告。

## 检查点

- 本地 Linux 与 Windows target check 通过后才推送 workflow。
- workflow 失败不能进入 release job；runtime-verify 失败会使验证 workflow 失败。
- 不把无头 `--version` 结果写成 GUI 交互验证。

## 计划级风险

- macOS 和 Windows 只能依赖原生 runner 验证，当前本机没有这些桌面环境。
- 安装器工具和 runner 镜像可能变化；zip/tar 保留为最低回滚分发物。

## 完成标准

- 三平台 package job 成功，产物哈希可验证，安装/卸载 smoke 通过。
- preview tag 创建 prerelease，release assets 和 runtime report 均可下载。
- 上下文记录实际 run URL、未覆盖 GUI 风险和回滚点。

## 预期文件

- `src/main.rs`
- `crates/pinora-app/Cargo.toml`
- `crates/pinora-app/src/{lib.rs,os_instance.rs,tray.rs,capture_kde.rs,kwin_place.rs,image_sink.rs,hotkey.rs}`
- `packaging/` 下平台打包脚本与清单
- `.github/workflows/ci.yml`
- `.github/workflows/package.yml`
- `.github/workflows/runtime-verify.yml`
- `AGENTS.md`、`.context/system/{overview,conventions,risks}.md`

## 验收标准

- `cargo check --workspace`、Windows target 检查和 macOS runner 检查通过。
- 三平台均上传原始二进制、安装包和 `SHA256SUMS.txt`，包内版本与二进制版本一致。
- Linux `.deb`/`.rpm`/`.tar.gz`、macOS `.app`/`.dmg`、Windows `.zip`/NSIS `.exe` 至少有一种安装包通过 runner-safe 安装启动卸载 smoke。
- tag 构建自动创建 GitHub pre-release；runtime-verify 汇总报告回写 release notes，失败则 workflow 失败。
- 未验证的真实 GUI 能力和风险在上下文中保留，不以静态构建结果替代。

## 验证

- 本地：`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、Windows target check、`git diff --check`、ctx validate。
- GitHub Actions：手动运行 `ci.yml` 与 `package.yml`，再运行 `runtime-verify.yml`；读取每个平台 job 和产物清单。
- 发布：推送 `v0.1.0-preview.1`，确认 GitHub Release 标记为 prerelease 且包含校验文件与运行时报告。

## 风险与回滚

- 风险：GitHub runner 的 GUI 进程只能做启动存活探针，不能证明屏幕捕获和剪贴板权限；报告中明确标注。
- 风险：安装器工具链或 runner 镜像变化导致格式不可用；workflow 保留压缩包作为最低可回滚分发物。
- 回滚：删除 038 workflow/packaging 与平台 adapter，恢复 036/037 主路径；不删除既有数据和用户配置。

## 完成记录

- 待实现。
