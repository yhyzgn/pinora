# 任务 098：跨平台发布流水线验证

- 状态：已完成
- 计划：`.context/plans/098_release_pipeline_validation.md`
- 规模：大
- 依赖：现有 CI、package、runtime-verify workflow；GitHub 发布权限。
- 生产行为变更：是；会触发 GitHub Actions，并在验证后创建 Pinora pre-release 与跨平台安装包资产。

## 任务目标

把当前代码提交通过 GitHub 原生三平台打包与安装 smoke 验证，并发布一个可追溯的 pre-release。

## 范围

- 审查与必要时修复 `.github/workflows/{package,runtime-verify}.yml` 和 `packaging/` 脚本。
- 以 `workflow_dispatch` 验证当前 `main` 的三平台 package artifact。
- 以唯一 `v0.1.0-preview.N` tag 触发 package/release，再验证 `runtime-verify` 的三平台报告。
- 更新上下文、风险、计划与任务完成记录。

## 非目标

- 不实施代码签名、公证、自动更新、真实 GUI E2E、截图权限探针、tray/热键/任务栏/Dock 验收。
- 不修改 Pinora 核心产品功能或引入新网络服务。

## 预期文件

- `.github/workflows/{package.yml,runtime-verify.yml}`（仅在实际 run 证明需要时）
- `packaging/{package-unix.sh,package-windows.ps1,pinora.nsi}`（仅在实际 run 证明需要时）
- `AGENTS.md`
- `.context/plans/098_release_pipeline_validation.md`
- `.context/tasks/098_release_pipeline_validation.md`
- `.context/system/{overview.md,risks.md,conventions.md}`

## 验收标准

1. 三平台 package run 生成可下载 artifact 与平台 SHA-256 清单，且任何一个平台失败都阻止 release。
2. tag 触发的 release 是 pre-release，资产来自同一成功 run，含 Linux/macOS/Windows 包和总 `SHA256SUMS.txt`。
3. runtime-verify 对同一 package run 的包完成 `--version` 与适用的安装/卸载 smoke，并将报告写入 release body。
4. 日志和提交不包含 token、个人路径、截图、OCR 或剪贴板内容；不把 runner smoke 声称为真实 GUI 验收。

## 验证

- `cargo run --quiet -- --version`
- `gh run view <ci-run-id> --log-failed`
- `gh workflow run package.yml --ref main -f version=<preview-version>`
- `gh run watch <package-run-id> --exit-status`
- `gh run view <package-run-id> --json jobs,artifacts,conclusion,url`
- `gh release view <preview-tag> --json isPrerelease,assets,url`
- `gh run view <runtime-verify-run-id> --log-failed`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：hosted runner 测试不涵盖真实桌面与签名/公证。缓解：release 明确是 pre-release，风险文档不扩大证明范围。
- 风险：release 或 runtime workflow 因权限、artifact 可见性、包脚本或 runner 环境失败。缓解：保留失败日志，最小修复后以新 run 重试。
- 回滚：回退 workflow/脚本改动；已发布 preview 只可更新说明、添加修复版或标记为弃用，不删除证据。

## 完成记录

- 2026-08-03 完成。CI `30783363209`、tag package/release `30783568639` 和 `runtime-verify` `30783727003` 均成功。
- 已发布 `v0.1.0-preview.8`：`https://github.com/yhyzgn/pinora/releases/tag/v0.1.0-preview.8`，`isPrerelease=true`，含 Linux/macOS/Windows 资产及总 `SHA256SUMS.txt`。
- 已下载 Release 全部 11 个资产并按总清单逐项执行 SHA-256 校验，结果全部通过；runtime 报告已回写 Release body。
- 实际验证范围止于 runner-safe 包结构、安装/卸载与 `--version`；未将其描述为真实桌面 GUI、tray-only、热键、权限、任务栏/Dock/分页器或流畅性验收。
- 回滚点：本阶段无源码、工作流或打包脚本变更；若后续运行验证失败，保留现有 tag/run 证据并以新的 preview tag 修复发布。
