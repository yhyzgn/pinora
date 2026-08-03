# 任务 106：捕获功能 crate

- 状态：已完成
- 计划：`.context/plans/106_capture_crate.md`
- 规模：中
- 依赖：任务 105 的 `pinora-platform` 已完成；现有 `pinora-core::CaptureProvider` 契约。
- 生产行为变更：否；这是内部所有权迁移。

## 任务目标

按捕获功能边界建立 `pinora-capture`，迁移 `capture_fake`、`capture_kde`、`capture_select`、`capture_xcap` 和 `frame_cache`，保持 app 的兼容导出与运行时行为。

## 变更前记录

```text
目的：让截图后端和预截帧缓存拥有独立、可测试且不依赖 UI 的 crate 边界。
影响路径：Cargo workspace、pinora-app 的模块导出、runtime/desktop_shell/history_load_job 的导入、捕获模块源码与上下文文档。
兼容性：接口 / 数据 / 状态 / 租户 / 权限均不改变；保持 CaptureProvider、CaptureRequest、CaptureImage、Unavailable 和 FrameCache 代际语义。
外部副作用：无新增系统注册、网络或共享基础设施访问；真实捕获只保留原有 KDE/xcap 探测。
回滚点：恢复 pinora-app 的五个模块和直接依赖，移除 pinora-capture。
验证场景：provider 选择、显示器/窗口快照二次校验、AllDisplays、缓存暂停/恢复/陈旧帧拒绝、workspace 和 Windows target。
```

## 范围

- 新增 `crates/pinora-capture/{Cargo.toml,src/*.rs}`。
- 更新根 `Cargo.toml`、`Cargo.lock`、`crates/pinora-app/Cargo.toml` 与 `src/lib.rs`。
- 更新 `runtime.rs`、`desktop_shell.rs`、`history_load_job.rs` 的捕获类型导入。
- 更新设计文档与 `.context/system/{overview,conventions,risks}.md` 的当前事实和验证记录。

## 非目标

- 不修改捕获 DTO、显示器拓扑算法、窗口策略、帧像素转换算法或桌面交互。
- 不引入新的截图后端、权限申请流程或生产 fake 降级。

## 预期文件

- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-capture/Cargo.toml`
- `crates/pinora-capture/src/{lib,capture_fake,capture_kde,capture_select,capture_xcap,frame_cache}.rs`
- `crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/{lib,runtime,desktop_shell,history_load_job}.rs`
- `AGENTS.md`、`.context/plans/106_capture_crate.md`、`.context/tasks/106_capture_crate.md`
- `docs/Pinora-开发设计文档.md`、`.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-capture` 对 `pinora-core` 单向依赖，不依赖 app/UI/jobs/storage/platform。
2. app 不再声明或编译 `capture_fake.rs`、`capture_kde.rs`、`capture_select.rs`、`capture_xcap.rs`、`frame_cache.rs`，但兼容 re-export 仍可用。
3. 既有捕获和 FrameCache 测试全部迁移且通过；不增加 ignored 以掩盖回归。
4. workspace 严格门禁、Windows target 和 ctx 校验通过；真实桌面证据仍如实标为未验证。

## 验证

- `cargo test -p pinora-capture -- --nocapture`
- `cargo test -p pinora-app runtime -- --nocapture`
- `cargo test -p pinora-app desktop_shell -- --nocapture`
- `cargo test -p pinora-app history_load_job -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：条件依赖漏接、re-export 漏接或 FrameCache 泛型路径变化导致编译/生命周期回归。
- 回滚：恢复 app 模块声明和依赖，删除新 crate；不改用户设置、历史文件和截图协议。

## 完成记录

- 已新增 `pinora-capture` crate，并将五个捕获/缓存模块的实现与测试迁移为唯一所有者；`pinora-app` 通过 `pub use pinora_capture` 保持兼容导出。
- 已更新 workspace、app manifest 及 runtime、desktop shell、history load job、capability probe 的导入；app 的直接依赖树不再包含 `xcap`。
- 已通过 `cargo test -p pinora-capture -- --nocapture`（25 通过、1 忽略）、`cargo test -p pinora-app runtime -- --nocapture`（18 通过）、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（根 1、app 261、capture 25、core 90、platform 21 通过；app/capture 各 1 个真实桌面测试忽略）、`cargo check --workspace`、严格 Clippy、Windows target、fmt、diff 和 `ctx validate`。
- 真实桌面捕获、权限、HiDPI、任务栏/Dock 和性能仍是开放风险。
