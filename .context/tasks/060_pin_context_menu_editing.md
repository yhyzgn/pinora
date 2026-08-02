# 任务 060：贴图上下文菜单与原位编辑

- 状态：已完成
- 计划：`.context/plans/060_pin_context_menu_editing.md`
- 规模：大
- 依赖：`.context/tasks/053_pin_render_cache.md`、`.context/tasks/054_auxiliary_window_boundary.md`、`.context/tasks/058_tray_residency_capture_failures.md`
- 生产行为变更：是；右键贴图在自身窗口内显示操作菜单，编辑可原位替换同一贴图的图片。

## 任务目标

在不新建系统菜单窗口或孤立贴图窗口的前提下，完成贴图右键高频操作和可回到标注 Overlay 的原位编辑闭环，并保持既有 `PinId`、后台任务归属与 tray 驻留生命周期。

## 范围

- 新增贴图图像替换、锁定与置顶领域/命令事务及回归测试。
- 添加独立的贴图上下文菜单布局、命中和绘制模块，并接入 `desktop_shell` 的贴图事件。
- 接入复制、OCR、编辑、锁定、透明度、置顶、另存和关闭；编辑基于既有 Overlay 并原位更新 `PinId`。

## 非目标

- 不增加系统窗口或原生菜单，不实现跨平台真实置顶确认、文件选择器、点击穿透、撤销关闭、无障碍树或历史持久化迁移。

## 预期文件

- `crates/pinora-core/src/{command.rs,state.rs,pin.rs}`
- `crates/pinora-app/src/{desktop_shell.rs,pin_context_menu.rs,runtime.rs}`
- `crates/pinora-app/src/lib.rs`
- `AGENTS.md`
- `.context/plans/060_pin_context_menu_editing.md`
- `.context/tasks/060_pin_context_menu_editing.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 菜单在贴图自身客户区内打开/关闭、布局不越界且只返回已启用动作。
2. 锁定贴图仍允许菜单、复制、OCR 和关闭，禁止受限编辑/透明度变换。
3. 编辑提交保留 `PinId`、替换领域图片、释放旧无引用图像、推进资产 generation 并拒绝旧任务结果；取消恢复贴图。
4. 置顶切换只请求现有窗口级别并保持 `window_policy` 任务栏/Dock 隔离，不创建新窗口。
5. 定向测试、fmt、workspace check、严格 Clippy、全量测试、diff 检查、`ctx validate` 和 GitHub 三平台 CI 通过。

## 验证

- `cargo test -p pinora-core state -- --nocapture`
- `cargo test -p pinora-app pin_context_menu::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：自绘菜单遮挡小贴图或事件竞争。缓解：限制到客户区并由纯布局/命中测试覆盖；无命中时关闭菜单。
- 风险：编辑替换后陈旧 OCR/导出结果回写。缓解：关闭 pin owner、推进 `AssetRef` generation，再开始新任务。
- 风险：不同合成器不接受窗口层级变更。缓解：请求真实 `WindowLevel`，保留状态为请求值并记录原生验证缺口。
- 回滚：移除菜单/编辑入口和替换调用，既有键盘贴图操作与窗口策略保持不变。

## 完成记录

- 已完成：新增 `ReplacePinImage`、`SetPinLocked`、`SetPinAlwaysOnTop` 命令；菜单在贴图客户区内绘制/命中，锁定状态禁用编辑和压暗，不触发受限状态改写。
- 已完成：编辑 Overlay 经 `window_policy::create_auxiliary_window` 创建；提交保持同一 `PinId`，关闭旧 owner 任务、推进 generation、更新尺寸/缓存/可见性；取消、失败和重新截图恢复原贴图。
- 已验证：领域、菜单、Overlay 比例与编辑命令定向测试，以及 fmt、workspace check、严格 Clippy、全量离线测试和 diff 检查均已通过。
- 已知风险：真实任务栏/Dock、KWin、HiDPI、菜单输入延迟和置顶结果未由 GUI 会话验证；当前结论只覆盖代码契约与离线测试。
