# 任务 109：桌面交互原语 crate

- 状态：已完成
- 计划：`.context/plans/109_desktop_primitives.md`
- 规模：中
- 依赖：任务 061 tray-only 窗口入口、任务 068 Overlay 预览缓存、任务 108 本地存储 crate。
- 生产行为变更：否；内部 crate 所有权迁移。

## 变更前记录

```text
目的：把贴图几何、Overlay 工具栏和已提交预览缓存从桌面壳单体中剥离，先建立无窗口句柄的可测试边界。
影响路径：workspace、pinora-app 的 desktop_shell/lib 导入、设计文档和上下文事实。
兼容性：接口 / 数据 / 状态 / 租户 / 权限均不改变；保持现有图像像素、坐标、快捷键和缓存门禁。
外部副作用：无新增窗口、线程、进程、网络、文件或系统注册。
回滚点：恢复 app 内三个模块和直接引用，移除 pinora-desktop。
验证场景：贴图缩放/定位/八方向调整、工具栏布局/命中/窄画布、缓存按选区与 revision 失效及草稿合成。
```

## 任务目标

在不改变现有窗口和事件循环所有权的前提下，把三个纯桌面交互原语迁移至独立 crate，形成后续 Overlay/贴图窗口拆分可复用的稳定数据边界。

## 范围

- 新增 `crates/pinora-desktop/{Cargo.toml,src/{lib,pin_layout,overlay_toolbar,overlay_preview_cache}.rs}`。
- 更新 workspace、app manifest、`desktop_shell.rs` 导入、app `lib.rs` 兼容 re-export。
- 更新设计文档与 `.context/system/{overview,conventions,risks}.md`。

## 预期文件

- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-desktop/Cargo.toml`、`crates/pinora-desktop/src/{lib,pin_layout,overlay_toolbar,overlay_preview_cache}.rs`
- `crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/{lib,desktop_shell}.rs`
- `AGENTS.md`、`.context/{plans,tasks}/109_desktop_primitives.md`
- `docs/Pinora-开发设计文档.md`、`.context/system/{overview,conventions,risks}.md`

## 非目标

- 不迁移窗口策略、托盘、事件循环、面板窗口、OCR、导出或历史清理。
- 不改 `pinora-core` 数据模型和任何生产行为。

## 验收标准

1. `pinora-desktop` 仅依赖 `pinora-core` 与标准库，拥有三模块唯一实现和原有测试。
2. app 不再声明或编译旧模块；现有公共 re-export 和 desktop shell 行为保持兼容。
3. 定向测试、workspace、严格 Clippy、Windows target、fmt、diff 和 ctx 校验通过。
4. 真实桌面窗口、tray、HiDPI、合成器与帧时间仍按风险记录，不被离线证据冒充。

## 风险与回滚

- 风险：新 crate 的公开原语 API 被窗口适配误用，或迁移后的 import 遗漏导致行为路径分叉。
- 回滚：恢复 app 内三个模块和直接导入，移除 workspace/app 对 `pinora-desktop` 的依赖；不触碰用户文件或领域数据。

## 验证

- `cargo test -p pinora-desktop -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 完成记录

- 已迁移三个模块和全部原有测试，`OverlayPreviewCache` 的跨 crate 调用接口显式公开；app 兼容导出和唯一事件循环保持不变。
- 已通过新 crate 定向测试 25 项、workspace 全量离线测试、workspace check、严格 Clippy、Windows target check、fmt、diff check 和 `ctx validate`。
- 真实桌面窗口、tray、HiDPI、合成器和帧时间尚未验证，继续由后续窗口适配任务跟踪。
