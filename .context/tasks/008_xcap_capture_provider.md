# 任务 008：实现 XcapCaptureProvider 与启动降级

- 状态：已完成
- 计划：`.context/plans/008_real_capture.md`
- 规模：中
- 依赖：`.context/tasks/007_export_hotkey_actions.md`
- 生产行为变更：有

## 任务目标

实现基于 xcap 的显示器枚举与区域/全屏捕获，main 启动优先真捕获、失败回退 fake，并更新能力探测说明。

## 范围

- `pinora-app`：`capture_xcap.rs`、provider 选择、`StaticCapabilityProbe` 或等价。
- 依赖：`xcap`。
- 文档/上下文同步。

## 非目标

- Overlay、系统剪贴板、全局热键。

## 预期文件

- `crates/pinora-app/src/capture_xcap.rs`
- `crates/pinora-app/src/{lib,platform,main 调用侧}.rs`
- `crates/pinora-app/Cargo.toml`
- `.context/*`、`AGENTS.md`

## 验收标准

- 单元测试不调用真实 xcap（或 `#[ignore]`）。
- 真捕获成功时 DisplayInfo 来自 monitor 几何。
- 捕获失败时应用仍可 bootstrap 并使用 fake。

## 验证

- `cargo test --workspace`
- `cargo run` 观察 capability note 为 xcap 或 fake

## 风险与回滚

- 风险：xcap 拉入大量原生依赖导致构建变慢/失败。缓解：锁定版本，失败则文档记录。
- 回滚：移除 xcap 依赖，恢复仅 FakeCaptureProvider。

## 完成记录

- 状态：已完成（2026-07-30）。
- 实际变更：`XcapCaptureProvider`、`SelectedCaptureProvider::autodetect`、RuntimeCapabilityProbe；系统安装 pipewire-devel/mesa-libgbm-devel。
- 实际验证：`cargo test --workspace` 通过（1 ignored）；本机 `cargo run` 使用 xcap，主屏 3840×2160，导出 320×180 PNG。
- 未解决项：Overlay 选区 UI、权限引导 UI。
