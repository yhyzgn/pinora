# 任务 038：跨平台构建与预发布交付

- 状态：进行中
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

- 待实现。
