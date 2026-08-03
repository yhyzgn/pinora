# 任务 133：Overlay 会话状态模块

- 状态：已完成
- 计划：`.context/plans/133_overlay_session_state.md`
- 规模：小
- 依赖：任务 030、031、032、069、070、071、123、125、129 已完成。
- 生产行为变更：否；Overlay 会话状态与资产身份的内部模块迁移。

## 任务目标

让 `pinora-app::overlay_session` 唯一拥有 Overlay 阶段、派生资产身份、revision 映射和图像身份盖章；
让 `desktop_shell` 保留 Overlay 窗口、绘制、输入、标注文档写入、OCR/导出任务和 EventLoop。

## 变更前记录

```text
目的：从 desktop_shell 抽出纯 Overlay 会话身份，降低唯一事件循环文件的职责密度。
影响路径：确认选区、重选、标注撤销/重做、Overlay OCR、复制和保存结果门禁。
兼容性：ImageId、AssetRef generation、选区阶段、标注 revision、窗口、任务 owner 与状态字符串不变。
外部副作用：无新增；Window/Surface、softbuffer、输入、绘制、任务、tray 和 EventLoop 保持原路径。
回滚点：恢复 desktop_shell 内定义并删除 overlay_session。
验证场景：标注 revision 变化、撤销/重做、重选新身份、图像盖章和全量回归。
```

## 范围

- 新增 `crates/pinora-app/src/overlay_session.rs`，迁移纯状态、映射和回归测试。
- 更新 `crates/pinora-app/src/{lib,desktop_shell}.rs`，删除重复定义。
- 更新 `AGENTS.md`、计划/任务、设计文档和 `.context/system/`。

## 非目标

- 不迁移 Overlay 窗口、Surface、绘制、winit 输入、标注文档、拖动状态、任务提交、OCR、导出、tray 或 EventLoop。
- 不新增依赖、网络、警告抑制或真实 GUI 测试。

## 预期文件

- `AGENTS.md`
- `.context/{plans,tasks}/133_overlay_session_state.md`
- `crates/pinora-app/src/{overlay_session,lib,desktop_shell}.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `overlay_session` 不依赖 winit，唯一拥有迁移的 Overlay 纯状态与资产映射。
2. `desktop_shell` 保留所有 Overlay 窗口、输入、绘制、标注与任务副作用，行为不变。
3. 定向测试、完整 workspace、Clippy、Windows target、fmt、diff 和 ctx validate 通过。

## 验证

- `cargo test -p pinora-app overlay_session -- --nocapture`
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

- 风险：身份迁移遗漏导致重选、撤销/重做或迟到 OCR/导出结果处理错误。
- 回滚：恢复 desktop_shell 内类型/函数并移除 overlay_session；不触碰窗口、图像、标注、任务、tray 或设置。

## 完成记录

- 2026-08-03：新增 `overlay_session`，从 `desktop_shell` 迁移 `OverlayPhase`、`OverlayAssetIdentity`、
  revision 到 `AssetRef` 映射与派生图像盖章；三项回归覆盖陈旧结果拒绝、撤销/重做 generation 和重选身份。
- `desktop_shell` 仅改为导入会话状态并委托资产映射；Window/Surface、绘制、输入、标注文档、任务、tray 与
  EventLoop 路径不变。未新增依赖或外部副作用。
- 已通过：`cargo test -p pinora-app overlay_session -- --nocapture`（3 项）、
  `cargo test -p pinora-app --lib -- --nocapture`（27 项）、
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo run --quiet -- --version`、
  `cargo fmt --all -- --check`、`git diff --check` 与 `ctx validate`。
