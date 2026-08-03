# 任务 124：捕获请求契约

- 状态：已完成
- 计划：`.context/plans/124_capture_request_contract.md`
- 规模：中
- 依赖：任务 106、120、122、123 已完成。
- 生产行为变更：否；内部请求契约所有权迁移。

## 任务目标

让 `pinora-capture` 唯一拥有截图模式、截图目标、Overlay 初始选区映射和显示器目标解析，
让 app 仅按该契约发起实际捕获与编排窗口生命周期。

## 变更前记录

```text
目的：将截图请求意图和显示器解析从 desktop_shell 迁入 pinora-capture，删除本地副本。
影响路径：热键、tray、IPC 发起的区域/全屏/全部显示器/窗口截图请求，以及 Overlay 初始选区。
兼容性：不改变接口、数据、状态、租户或权限语义；目标消失时继续拒绝，禁止回退到其他显示器。
外部副作用：无；请求契约不访问截图后端、窗口、线程、文件、网络或共享基础设施。
回滚点：恢复 desktop_shell 内的模式/目标/选区/解析值对象，移除 pinora-capture 对应导出。
验证场景：模式映射、目标日志、默认屏、显式屏、目标消失、非显示器目标、全图选区和 app 回归。
```

## 范围

- 新增 `crates/pinora-capture/src/capture_request.rs`。
- 迁移 `CaptureMode`、`CaptureTarget`、`OverlayInitialSelection` 及模式/目标/选区/显示器解析。
- 切换 app 的捕获请求与 Overlay 初始化路径，迁移相关测试。
- 更新 crate 导出、设计/系统/风险文档。

## 非目标

- 不改变真实截图后端、FrameCache、倒计时、失败恢复、Window/Surface、EventLoop、渲染、
  标注、OCR、导出、历史、贴图、热键、IPC 或托盘。
- 不改变用户可见截图结果、默认屏选择、错误码或 Overlay 初始选区。

## 预期文件

- `AGENTS.md`
- `.context/plans/124_capture_request_contract.md`
- `.context/tasks/124_capture_request_contract.md`
- `crates/pinora-capture/src/{lib,capture_request}.rs`
- `crates/pinora-app/src/{lib,desktop_shell}.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. capture crate 唯一拥有请求模式、目标、初始选区和显示器解析；app 删除本地同类实现。
2. 模式、目标、默认最大面积显示器、目标消失、非显示器拒绝和选区应用均由 capture 测试覆盖。
3. app 仍独占倒计时、失败恢复、实际捕获、Overlay、Window/Surface、softbuffer present、托盘和
   唯一 EventLoop。

## 验证

- `cargo test -p pinora-capture -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：模式或目标映射错误可造成错误截图范围、全图/手动选区回归，或在显示器消失时误回退。
- 回滚：恢复 app 内请求契约，移除 capture crate 对应导出；不触碰真实截图、窗口、输入、托盘、
  数据格式或导出。

## 完成记录

- 2026-08-03 已完成。新增 `pinora-capture::capture_request`，将 `CaptureMode`、
  `CaptureTarget`、`OverlayInitialSelection`、模式/目标标签、初始选区应用和显示器解析
  迁出 `desktop_shell`；区域保持手动选区，全屏/全部显示器/窗口保持全图选区，显式目标
  消失继续返回 `NotFound`，非显示器目标继续返回 `InvalidState`。

- 验证通过：`cargo test -p pinora-capture -- --nocapture`（33 通过，1 项真实显示会话
  测试忽略）、`cargo test -p pinora-app --lib -- --nocapture`（30 通过）、
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、
  `cargo run --quiet -- --version`（输出 `pinora 0.1.0`）、`cargo fmt --check`、
  `git diff --check` 与 `ctx validate`。

- 未覆盖风险：上述离线/交叉编译/版本探针不构成真实截图权限、窗口、任务栏/Dock、HiDPI、
  焦点或性能验收，继续由 R-075 跟踪。
