# 任务 132：贴图会话状态模块

- 状态：已完成
- 计划：`.context/plans/132_pin_session_state.md`
- 规模：小
- 依赖：任务 076、075、088、090、095、128、129、130、131 已完成。
- 生产行为变更：否；贴图会话值对象和纯状态转移的内部模块迁移。

## 任务目标

让 `pinora-app::pin_session` 唯一拥有贴图鼠标模式、创建呈现参数、关闭恢复快照和最近使用序号，
让 `desktop_shell` 继续独占贴图窗口、输入、平台调用、runtime、OCR、导出、tray 和 EventLoop。

## 变更前记录

```text
目的：从 desktop_shell 抽出无窗口副作用的贴图会话值对象，继续降低唯一事件循环文件的职责密度。
影响路径：贴图鼠标穿透、tray 重新唤起、创建、关闭、撤销关闭和最近使用排序。
兼容性：窗口、任务栏/Dock 策略、PinId、AssetRef、位置、缩放、不透明度、锁定、置顶、tray 文案与状态语义不变。
外部副作用：无新增；Window/Surface、平台命中、runtime 命令、OCR、导出、tray 和 EventLoop 行为保持原路径。
回滚点：恢复 desktop_shell 内类型/函数，移除 pin_session 模块及上下文记录。
验证场景：命中状态、成功/失败平台请求、最近使用饱和、关闭恢复字段和全量回归。
```

## 范围

- 新增 `crates/pinora-app/src/pin_session.rs`，迁移贴图会话值对象、纯转移和回归测试。
- 更新 `crates/pinora-app/src/{lib,desktop_shell}.rs`，删除重复定义并保持现有副作用调用。
- 更新 `AGENTS.md`、计划/任务、设计文档、overview、conventions 和 risks。

## 非目标

- 不迁移 Window/Surface、winit 输入、缩放/拖动、渲染、窗口策略、平台请求、runtime、OCR、导出、tray 或 EventLoop。
- 不新增依赖、原始 SQL、警告抑制、网络访问或真实 GUI/命中测试。

## 预期文件

- `AGENTS.md`
- `.context/plans/132_pin_session_state.md`
- `.context/tasks/132_pin_session_state.md`
- `crates/pinora-app/src/{pin_session,lib,desktop_shell}.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pin_session` 唯一拥有贴图会话值对象和纯状态转移，且不依赖 winit，不创建窗口、不启动线程、不调用平台、runtime、OCR、导出或 tray。
2. `desktop_shell` 保留贴图窗口、输入、真实鼠标命中请求、runtime、OCR、导出、tray 和所有外部副作用，现有用户语义不变。
3. 会话模块边界测试和 app 回归通过；workspace、Clippy、Windows target、fmt、diff 与 ctx validate 通过。

## 验证

- `cargo test -p pinora-app pin_session -- --nocapture`
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

- 风险：迁移遗漏导致鼠标穿透状态、关闭恢复字段或最近使用排序改变。
- 回滚：恢复 desktop_shell 内定义并删除 pin_session；不触碰贴图窗口、图像、runtime、OCR、导出、tray 或设置。

## 完成记录

- 实现：新增 `crates/pinora-app/src/pin_session.rs`，迁移 `PinMouseMode`、平台请求状态转移、
  `PinPresentation`、`ClosedPinSnapshot` 和 `next_pin_recency`。模块不依赖 winit，不创建窗口、
  不启动线程、不调用平台/runtime/OCR/导出/tray。
- 兼容性：平台拒绝鼠标命中请求时仍保持旧模式；关闭恢复快照继续只含图像、位置、缩放、不透明度、
  锁定和置顶，不含 Window/Surface、鼠标穿透或 worker。窗口、输入和所有副作用时机未改变。
- 验证：`cargo test -p pinora-app pin_session -- --nocapture`（3 通过）；
  `cargo test -p pinora-app --lib -- --nocapture`（27 通过）；
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、fmt、diff 与 `ctx validate` 均通过。
- 风险与回滚：R-083 继续跟踪真实鼠标命中、窗口管理器、焦点、任务栏/Dock、HiDPI、tray-only 和性能；
  回滚时恢复 `desktop_shell` 内值对象并移除 `pin_session`，不触碰贴图窗口、图像、runtime、OCR、导出、
  tray 或设置。
