# 任务 038：跨平台构建与预发布交付

- 状态：已完成
- 计划：`.context/plans/038_cross_platform_delivery.md`
- 规模：大
- 依赖：`.context/system/overview.md`、`.context/system/conventions.md`、`.context/system/risks.md`
- 生产行为变更：是；新增逐平台编译、安装包和发布流水线，但不改变已验证 Linux 主流程的业务状态语义。

## 变更前记录

```text
目的：在 GitHub Actions 完成 Linux/macOS/Windows 构建、安装包、安装 smoke 与 pre-release。
影响路径：Cargo target 依赖、单实例 IPC、平台能力模块、packaging、.github/workflows。
兼容性：保留 Unix activate.sock；非 Unix 使用 loopback TCP 端口文件；命令和领域状态不变。
外部副作用：仅访问 GitHub Actions、GitHub Release 和 runner 本地临时目录；不连接共享数据库、缓存、消息队列或第三方业务服务。
回滚点：移除 038 adapter、脚本和 workflow 即可恢复前一阶段入口。
验证场景：三平台编译、包哈希、安装/启动/卸载 smoke、tag prerelease、runtime-verify 报告。
```

## 任务目标

建立可审计的跨平台交付链：先使代码按 target 编译，再生成最小可运行分发物，最后通过 GitHub Actions 调试和发布。

## 范围

- target-specific GTK 与 Linux-only adapter。
- 跨平台单实例转发和 CLI 版本探针。
- `packaging/` 打包脚本、桌面清单、版本和校验文件。
- `ci.yml`、`package.yml`、`runtime-verify.yml`。

## 非目标

- 真实 GUI 交互、多显示器、HiDPI、系统剪贴板和屏幕捕获权限的自动化证明。
- 历史 UI/删除事务和 037 导出接入。

## 预期文件

- `src/main.rs`、`crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/` 平台边界模块。
- `packaging/package-unix.sh`、`packaging/package-windows.ps1`、`packaging/pinora.nsi`。
- `.github/workflows/ci.yml`、`.github/workflows/package.yml`、`.github/workflows/runtime-verify.yml`。
- `AGENTS.md` 与 `.context/system/` 验证记录。

## 验收标准

- 三平台 job 成功且产物存在；失败日志可定位到平台步骤。
- 每个包均可解包/安装并执行 `pinora --version`，卸载不残留安装目录。
- 发布版本是 GitHub prerelease，release assets 含二进制、安装包、`SHA256SUMS.txt` 和 runtime report。

## 验证

- 本地 `cargo fmt --check`、`cargo check --workspace`、Windows target check、严格 Clippy、workspace tests、`git diff --check` 和 ctx validate。
- GitHub Actions `ci.yml`、`package.yml`、`runtime-verify.yml` 的真实 run 输出和 artifact 下载。

## 验证与记录

- 本地质量门禁与 ctx validate。
- `gh workflow run`、`gh run watch`、`gh run download` 读取实际结果并回写完成记录。

## 风险与回滚

- 包管理器在 runner 上不可用时保留 tar/zip/app bundle 作为最低分发物，并在 release notes 标注。
- 任何未通过 smoke 的平台禁止标记为已验证；tag 发布前先修复 workflow。

## 完成记录

- 已完成（2026-08-02）：
  - 代码与流水线提交已推送至 `origin/main`，最新相关提交为 `8bc4a47`；GitHub CI run `30714522975` 成功。
  - `v0.1.0-preview.2` package run `30714649807` 成功，发布 job `91408449669` 成功；Release URL：<https://github.com/yhyzgn/pinora/releases/tag/v0.1.0-preview.2>。
  - runtime-verify run `30715042640` 成功，验证 job 覆盖 `ubuntu-24.04`、`macos-14`、`windows-2022`，报告已回写 Release notes。
  - Release 下载后的 `SHA256SUMS.txt` 对 10 个分发资产逐项校验通过；当前 Release 为 prerelease，raw binary 与安装包版本均为 `0.1.0-preview.2`。
  - 未覆盖项已明确记录：没有把无头 runner 结果描述为真实桌面交互或平台能力等价验证；签名/公证、真实权限、多显示器、HiDPI、屏幕捕获、剪贴板、热键和托盘仍需对应桌面环境验收。
