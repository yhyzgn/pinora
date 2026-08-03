# 任务 125：Overlay 输入意图契约

- 状态：已完成
- 计划：`.context/plans/125_overlay_input_intents.md`
- 规模：小
- 依赖：任务 109、120、121、124 已完成。
- 生产行为变更：否；内部输入语义所有权迁移。

## 任务目标

让 `pinora-desktop` 唯一拥有 Overlay 键盘和鼠标输入意图判定，让 app 继续拥有事件分发和
所有可观察状态变更。

## 变更前记录

```text
目的：将 Overlay 的撤销/重做、文本 Enter、微调步长和双击复制判定从 desktop_shell 迁入 pinora-desktop。
影响路径：Overlay 标注的键盘输入、文本编辑、选中标注微调和选区内双击行为。
兼容性：不改变接口、数据、状态、租户或权限语义；序号/选择工具的双击继续不复制。
外部副作用：无；输入判定不创建窗口、不写文件、不启动任务、不访问网络或共享基础设施。
回滚点：恢复 desktop_shell 内输入意图枚举/函数，移除 pinora-desktop 对应导出。
验证场景：撤销、重做、文本换行、文本提交、1/10 像素步长、普通双击复制和豁免工具双击。
```

## 范围

- 新增 `crates/pinora-desktop/src/overlay_input.rs`。
- 迁移 Overlay 输入意图类型、键盘修饰键映射、微调步长和双击复制判定。
- 切换 app Overlay 事件路径，迁移相关测试。
- 更新 crate 导出、设计/系统/风险文档。

## 非目标

- 不改变 winit 事件读取、Overlay 状态、标注文档、工具栏、Window/Surface、EventLoop、捕获、
  OCR、导出、历史、贴图或托盘。
- 不改变任何用户可见快捷键、文本输入、双击行为或用户数据。

## 预期文件

- `AGENTS.md`
- `.context/plans/125_overlay_input_intents.md`
- `.context/tasks/125_overlay_input_intents.md`
- `crates/pinora-desktop/src/{lib,overlay_input}.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. desktop crate 唯一拥有 Overlay 输入意图、修饰键判定、微调步长和双击复制规则；app 删除本地副本。
2. 撤销/重做、文本、步长和双击豁免规则由 desktop 测试覆盖。
3. app 仍独占 winit 事件循环、Overlay 状态、标注文档、任务、Window/Surface、softbuffer present、托盘和唯一 EventLoop。

## 验证

- `cargo test -p pinora-desktop -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：修饰键或工具豁免映射错误会造成功能回归。
- 回滚：恢复 app 内输入判定，移除 desktop crate 对应导出；不触碰窗口、输入源、数据格式、截图、OCR、导出、历史或托盘。

## 完成记录

- 2026-08-03 已完成。新增 `pinora-desktop::overlay_input`，将
  `AnnotationHistoryAction`、`TextEnterAction`、撤销/重做、文本 Enter、微调步长和双击
  复制判定迁出 `desktop_shell`；Ctrl+Z、Ctrl+Shift+Z/Ctrl+Y、Shift+Enter、Enter、
  Shift+方向键及序号/选择工具的双击豁免均保持不变。

- 验证通过：`cargo test -p pinora-desktop -- --nocapture`（95 通过）、
  `cargo test -p pinora-app --lib -- --nocapture`（26 通过）、
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、
  `cargo run --quiet -- --version`（输出 `pinora 0.1.0`）、`cargo fmt --check`、
  `git diff --check` 与 `ctx validate`。

- 未覆盖风险：上述离线/交叉编译/版本探针不构成真实输入法、焦点、窗口、任务栏/Dock、
  HiDPI 或性能验收，继续由 R-076 跟踪。
