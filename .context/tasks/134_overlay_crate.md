# 任务 134：Overlay 会话 crate

- 状态：已完成
- 计划：`.context/plans/134_overlay_crate.md`
- 规模：小
- 依赖：任务 133 已完成。
- 生产行为变更：否；Overlay 纯会话模块的 crate 边界迁移。

## 任务目标

新建 `pinora-overlay`，唯一承载 `OverlayPhase`、`OverlayAssetIdentity`、revision 到 `AssetRef` 的映射和
派生图像身份盖章；让 `pinora-app` 只消费该 crate，并保持桌面壳的真实副作用不变。

## 变更前记录

```text
目的：将已经稳定的 Overlay 纯会话从 app 私有模块提升为明确功能 crate，继续降低 desktop_shell 的职责密度。
影响路径：确认选区、重选、标注撤销/重做、Overlay OCR、复制与保存的资产结果门禁。
兼容性：OverlayPhase、ImageId、AssetRef generation、标注 revision、窗口、任务 owner 和状态字符串不变。
外部副作用：无新增；Window/Surface、softbuffer、输入、绘制、任务、tray 和 EventLoop 保持原路径。
回滚点：移除 pinora-overlay 并恢复 pinora-app 内部模块。
验证场景：revision 变化、撤销/重做、重选新身份、图像盖章、workspace 依赖图与全量回归。
```

## 范围

- 新增 `crates/pinora-overlay`，迁移纯会话实现和三项回归测试。
- 更新 root workspace、`pinora-app` 依赖、模块声明与桌面壳导入。
- 更新 `AGENTS.md`、计划/任务、设计文档与 `.context/system/`。

## 非目标

- 不迁移 Overlay 窗口、Surface、绘制、winit 输入、标注文档、拖动状态、任务提交、OCR、导出、tray 或 EventLoop。
- 不新增第三方依赖、网络、警告抑制或真实 GUI 测试。

## 预期文件

- `AGENTS.md`
- `Cargo.toml`
- `crates/pinora-overlay/{Cargo.toml,src/lib.rs}`
- `crates/pinora-app/{Cargo.toml,src/lib.rs,src/desktop_shell.rs}`
- `.context/{plans,tasks}/134_overlay_crate.md`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-overlay` 生产依赖仅为 `pinora-core`，唯一拥有迁移的类型、映射与测试。
2. `pinora-app` 不再有 `overlay_session` 内部模块；桌面壳的窗口、输入、绘制、标注、任务和 EventLoop 行为不变。
3. 定向测试、workspace、Clippy、Windows target、fmt、diff 和 ctx validate 通过。

## 验证

- `cargo test -p pinora-overlay -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo run --quiet -- --version`
- `cargo fmt --all -- --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：可见性或依赖迁移错误导致资产身份语义改变，或把窗口/任务类型泄漏到功能 crate。
- 回滚：移除 workspace 成员与 app 依赖，将实现恢复为 app 私有模块；不触碰窗口、图像、标注、任务、tray 或设置。

## 完成记录

- 2026-08-03：新增 `crates/pinora-overlay`，迁移 `OverlayPhase`、`OverlayAssetIdentity`、revision 到
  `AssetRef` 映射和派生图像盖章；迁移三项回归，覆盖陈旧结果拒绝、撤销/重做 generation 与重选身份。
- root workspace 与 `pinora-app` 已接入新 crate，app 内部 `overlay_session` 已删除；生产依赖只包含
  `pinora-core`，`pinora-jobs` 仅作为测试依赖。为满足公开构造器的严格 Clippy 契约，补充行为等价的
  `Default` 实现。
- 已通过：`cargo test -p pinora-overlay -- --nocapture`（3 项）、
  `cargo test -p pinora-app --lib -- --nocapture`（24 项）、
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo run --quiet -- --version`、
  `cargo fmt --all -- --check`、`git diff --check` 与 `ctx validate`。
