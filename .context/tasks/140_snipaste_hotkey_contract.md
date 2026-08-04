# 任务 140：Snipaste 默认快捷键契约

- 状态：已完成
- 计划：`.context/plans/140_snipaste_hotkey_contract.md`
- 规模：中
- 依赖：任务 081、084、086、100、114、139 已完成。
- 生产行为变更：是；默认全局键位、剪贴板图像导入与全部贴图显隐行为改变。

## 任务目标

交付 F1 区域截图、F3 剪贴板贴图、Shift+F3 全部贴图显隐的统一生产契约。全屏截图保持 tray/IPC
入口；旧设置中的全屏键位不得被转换为 F3 剪贴板动作。

## 变更前记录

```text
目的：将用户要求的 Snipaste 核心快捷键变成可迁移、可取消、可验证的应用契约。
影响路径：ActionId、设置 codec、global-hotkey、Wayland Portal、聚焦窗口键盘、tray、剪贴板子进程、PinWindow 显隐。
兼容性：旧设置记录按新 schema 重置历史全屏键位；全屏截图保留 tray/IPC；状态与窗口策略不变。
外部副作用：F3 受控读取用户系统剪贴板；不联网、不写入用户剪贴板。
回滚点：移除 clipboard read service 与新增动作路由，保留迁移解码与 tray/IPC 全屏入口。
验证场景：动作映射、设置 round-trip/migration、PNG 解码边界、worker 身份/取消、app 路由、workspace 门禁。
```

## 范围

- `pinora-core` 动作、默认键位、设置 schema v10 和迁移。
- `pinora-platform` 原生热键、Wayland Portal、Linux desktop entry 和聚焦窗口键位映射。
- `pinora-export` 系统剪贴板 PNG 读回、解码和受监督 worker。
- `pinora-app` F1/F3/Shift+F3 编排、全部贴图显隐和剪贴板贴图复用。
- 设计文档、系统事实、风险和验证记录。

## 非目标

- 全屏截图能力本身、截图后端、贴图渲染和历史/OCR 行为。
- 真实共享桌面、剪贴板、Portal、任务栏/Dock/分页器或性能的线上环境探针。
- Windows/macOS 新的系统剪贴板读回实现。

## 预期文件

- `AGENTS.md`
- `src/main.rs`
- `crates/pinora-{core,storage,platform,desktop,export,tray,runtime,app}/src/`
- `.context/{plans,tasks}/140_snipaste_hotkey_contract.md`
- `.context/system/{overview,risks}.md`
- `docs/Pinora-开发设计文档.md`

## 验收标准

1. 默认设置与 v1-v9 迁移输出 F1/F3，旧全屏键位不再注册为剪贴板动作；v10 设置 round-trip 保持用户组合。
2. X11、Wayland Portal 和窗口内键盘路径产生三个稳定 `ActionId`，且 Shift+F3 不会被当作 F3。
3. F3 读取 `image/png` 不阻塞事件循环，拒绝超大、损坏、错配身份或非图像内容，成功后复用既有贴图窗口策略。
4. Shift+F3 只显隐已有窗口，不产生新资产、历史、任务栏/Dock/分页器入口。
5. 定向、workspace、Clippy、Windows、fmt、diff 和 ctx validate 均通过；真实桌面风险明确记录。

## 验证

- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --all -- --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：真实系统剪贴板、Portal、热键冲突与窗口管理器显隐可能与离线契约不同。
- 回滚：移除 clipboard read worker、`PasteClipboard` 与 `ToggleAllPinsVisibility` 路由；保留 F1 区域截图、tray/IPC 全屏、既有贴图、历史和窗口策略。

## 完成记录

- 已完成：`ActionId` 新增 `PasteClipboard` 与 `ToggleAllPinsVisibility`；默认 F1、F3、Shift+F3 分别且仅
  路由到区域截图、从剪贴板创建贴图、全部贴图显隐。全屏截图保留 tray/IPC，不再注册为默认全局快捷键。
- 已完成：设置 schema v10 将 v1-v9 的旧全屏键位迁移为默认 F1/F3，当前 v10 设置仍保留用户已录制的
  区域与剪贴板键位；Shift+F3 是固定键，且不允许被这两项录制配置占用。`Ctrl+N`、`Ctrl+Shift+S` 已移除。
- 已完成：`ClipboardImageReadJobService` 在 worker 内以受限 `image/png` 读回、临时文件承接、超时、
  字节/像素/解码缓冲限制、取消与 `JobId`/owner/asset identity 门禁完成 F3；成功结果复用既有
  `open_pin_from_image` 和 `window_policy`。Shift+F3 只修改既有 `PinWin` 可见性。
- 已验证：`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --all -- --check`、
  `git diff --check` 与 `ctx validate` 全部通过。
- 已知风险：用户已确认不会再运行 Snipaste，故不存在该应用的快捷键冲突；真实系统的其他占用、剪贴板/Portal
  权限、窗口管理器显隐、tray-only 隔离、焦点、HiDPI 和性能未作原生桌面验证，R-086 持续跟踪。
- 回滚边界：可移除 clipboard read worker 与两项动作路由，保留 F1 区域截图、tray/IPC 全屏入口、既有
  系统剪贴板写入、贴图、历史、设置文件和窗口策略。
