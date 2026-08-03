# 计划 098：跨平台发布流水线验证

- 状态：进行中
- 负责人：Codex
- 当前任务：`.context/tasks/098_release_pipeline_validation.md`

## 目标

以 GitHub 原生 Linux、macOS 和 Windows runner 的真实产物验证 Pinora 的打包、校验、安装 smoke 与 pre-release 发布链路；修复已证实的工作流或安装脚本缺陷，并只在三平台包成功后发布预览版本。

## 非目标

- 不把 runner 的 `--version`、安装或卸载 smoke 描述为真实截图、tray、热键、窗口、任务栏/Dock/分页器、权限或流畅性验证。
- 不新增自动更新、代码签名证书、公证、云端服务、遥测、网络上传或权限绕过。
- 不改变截图、标注、贴图、OCR、历史和设置的业务语义。

## 依赖关系

- 依赖现有 `ci.yml`、`package.yml`、`runtime-verify.yml` 与 Linux/macOS/Windows 打包脚本。
- 依赖 `--version` 不创建 tray、窗口、配置或单实例锁，适合作为 runner 安装 smoke。
- 依赖 GitHub token 具备已授权的工作流、tag、release 与 artifact 读取权限；用户已明确授权调试构建、打包和 pre-release 发布。

## 约束

- 先等待当前提交的 CI 成功，再以 `workflow_dispatch` 在 `main` 上运行无发布的三平台 artifact 验证；失败只根据实际 run 日志修改对应工作流/脚本。
- release job 只能从同一 package run 的三个成功 artifact 提取文件，逐平台 `SHA256SUMS.txt` 验证后重建 release 总清单；不得发布缺任一平台的部分产物。
- 创建唯一的 `v0.1.0-preview.N` annotated tag 并推送，触发 tag 发布；发布必须为 pre-release，且 release 资产只包含已验证的包与总校验清单。
- `runtime-verify.yml` 必须对同一 package run 的 Linux tar/deb、macOS zip 和 Windows zip/安装包运行 `--version`；失败必须保留报告并使 workflow 失败。
- 不将 `GITHUB_TOKEN`、认证信息或 token 值写入源码、日志、上下文或提交信息。

## 阶段

1. 检查当前 CI、CLI 行为、workflow permissions、产物名称和打包脚本输入输出。
2. 手动触发三平台 package workflow，等待完整 run，读取失败日志并最小修复。
3. 审查并再次运行 package workflow，验证每个平台 artifact、单平台清单和包内 `--version` smoke。
4. 创建并推送 pre-release tag，等待 package/release/runtime-verify，核对 GitHub Release 的 pre-release 标记、资产和 runtime 报告。
5. 更新上下文与风险，记录已验证发布证据和未覆盖的原生 GUI 验收范围。

## 检查点

1. `main` CI 成功，当前可执行文件 `--version` 以零副作用打印版本。
2. 手动 package run 三个平台均成功，artifact 命名唯一，清单可校验。
3. tag package run 的 release job 成功，Release 是 pre-release 且有 Linux/macOS/Windows 资产和总 `SHA256SUMS.txt`。
4. runtime-verify 读取对应 package run，三个 runner 的安装/`--version`/卸载 smoke 成功，release body 获得运行时报告。

## 计划级风险

- GitHub hosted runner 不能给出真实桌面会话、截图权限、tray、热键、任务栏/Dock 或性能证据；只验证可安装二进制与基础包结构。
- 无签名证书和 macOS notarization 时，Windows SmartScreen/macOS Gatekeeper 仍可能告警；不能把预发布资产描述为已签名或已公证。
- 发布 tag 和 release 是外部可见副作用；仅在用户授权的 preview 命名空间执行，失败时保留 run 与 asset 证据，不删除已发布版本以掩盖事实。

## 变更前记录

```text
目的：为当前 Pinora 提交建立真实三平台打包、安装 smoke 和预发布证据链。
影响路径：.github/workflows/{package,runtime-verify}.yml、packaging 脚本、Git tag/Release 与上下文。
兼容性：不改变公共 API、持久化数据、状态字符串、租户或权限语义。
外部副作用：触发 GitHub Actions；通过验证后创建 preview tag 与 GitHub pre-release，上传安装包和校验清单。
回滚点：工作流/脚本变更可回退；已发布 preview 只标记或更新，不能将其伪装为未发生。
验证场景：GitHub 原生 Linux/macOS/Windows 打包、artifact 清单、安装/--version/卸载 smoke、Release pre-release 属性和 runtime 报告。
```

## 完成标准

- 当前提交 CI、手动 package run、tag package/release 和 runtime-verify 都有成功 run 证据。
- GitHub pre-release 含三个平台的可下载资产及可验证的总 SHA-256 清单。
- 所有实际失败都有最小修复和复跑证据；没有把 runner smoke 扩大为 GUI 能力声明。

## 完成记录

- 待执行。
