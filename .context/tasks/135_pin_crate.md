# 任务 135：贴图会话 crate

- 状态：已完成
- 计划：`.context/plans/135_pin_crate.md`
- 规模：小
- 依赖：任务 132、134 已完成。
- 生产行为变更：否；贴图纯会话模块的 crate 边界迁移。

## 任务目标

新建 `pinora-pin`，唯一承载 `PinMouseMode`、平台确认后的状态转移、`PinPresentation`、
`ClosedPinSnapshot` 和最近使用序号；让 `pinora-app` 只消费该 crate，不改变桌面壳的真实副作用。

## 变更前记录

```text
目的：将稳定的贴图会话从 app 私有模块提升为明确功能 crate，继续降低 desktop_shell 的职责密度。
影响路径：贴图鼠标穿透、平台失败回退、动态 tray 排序、关闭/撤销关闭、贴图恢复。
兼容性：PinId、CaptureImage、呈现参数、鼠标穿透、最近使用排序、窗口、任务 owner 与状态字符串不变。
外部副作用：无新增；Window/Surface、输入、平台调用、OCR、导出、tray 和 EventLoop 保持原路径。
回滚点：移除 pinora-pin 并恢复 pinora-app 内部模块。
验证场景：平台成功/失败状态转移、序号饱和、关闭快照字段、workspace 依赖图与全量回归。
```

## 范围

- 新增 `crates/pinora-pin`，迁移纯会话实现和三项回归测试。
- 更新 root workspace、`pinora-app` 依赖、模块声明与桌面壳导入。
- 更新 `AGENTS.md`、计划/任务、设计文档与 `.context/system/`。

## 非目标

- 不迁移贴图窗口、Surface、绘制、winit 输入、平台鼠标命中调用、OCR、导出、tray 或 EventLoop。
- 不新增第三方依赖、网络、警告抑制或真实 GUI 测试。

## 预期文件

- `AGENTS.md`
- `Cargo.toml`
- `crates/pinora-pin/{Cargo.toml,src/lib.rs}`
- `crates/pinora-app/{Cargo.toml,src/lib.rs,src/desktop_shell.rs}`
- `.context/{plans,tasks}/135_pin_crate.md`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-pin` 生产依赖仅为 `pinora-core`，唯一拥有迁移的类型、状态转移与回归测试。
2. `pinora-app` 不再有 `pin_session` 内部模块；桌面壳的窗口、输入、平台调用、任务和 EventLoop 行为不变。
3. 定向测试、workspace、Clippy、Windows target、fmt、diff 和 ctx validate 通过。

## 验证

- `cargo test -p pinora-pin -- --nocapture`
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

- 风险：可见性或依赖迁移错误导致贴图瞬态状态语义改变，或把窗口/平台类型泄漏到功能 crate。
- 回滚：移除 workspace 成员与 app 依赖，将实现恢复为 app 私有模块；不触碰窗口、图像、OCR、导出、tray 或设置。

## 完成记录

- 已完成：新增 `pinora-pin` 并迁移 `PinMouseMode`、平台确认后的纯状态转移、`PinPresentation`、
  `ClosedPinSnapshot` 与饱和最近使用序号及其 3 项回归测试；root workspace 和 `pinora-app` 已接入
  crate，app 私有 `pin_session` 已删除。`desktop_shell` 仍独占窗口、输入、平台鼠标命中、OCR、导出、
  tray 和 EventLoop。
- 已验证：`cargo test -p pinora-pin -- --nocapture`（3 通过）、`cargo test -p pinora-app --lib -- --nocapture`
  （21 通过）、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、
  `cargo run --quiet -- --version`、`cargo fmt --all -- --check`、`git diff --check` 与 `ctx validate`
  均已通过。
- 风险：未运行真实 GUI；真实鼠标命中、窗口管理器、tray-only、任务栏/Dock、焦点、HiDPI 与性能仍未验证，
  由 R-083 跟踪。回滚时移除 `pinora-pin` workspace 成员与 app 依赖并恢复 app 私有模块，不触碰窗口、图像、
  OCR、导出、tray 或设置。
