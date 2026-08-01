# 任务 042：历史浏览与安全复用

- 状态：已完成
- 计划：`.context/plans/042_history_browser.md`
- 规模：大
- 依赖：`.context/tasks/034_history_index.md`、`.context/tasks/037_history_export_integration.md`、`.context/tasks/039_history_file_cleanup.md`、`.context/tasks/041_settings_panel.md`
- 生产行为变更：是；新增历史浏览、重新贴图和用户删除入口。

## 任务目标

让用户从桌面控制窗打开历史窗口，安全浏览应用管理的活动截图，选择后再次贴图或删除；不接受任意路径，也不把无效历史文件伪装为可复用截图。

## 范围

- 新增历史面板的纯状态、布局、键盘/鼠标命中和 XRGB 自绘。
- 新增受管 PNG 的长度、摘要、格式、尺寸和元数据校验/解码。
- 新增单条历史 tombstone 删除工作流与离线测试。
- 接入桌面 shell 的 H 快捷入口、预览、重新贴图和删除。

## 非目标

- 不实现全量清空、搜索、标签、OCR 结果回显、再次编辑或异步缩略图 worker。
- 不修改历史 codec、自动导出时机或用户导出文件管理边界。

## 预期文件

- `crates/pinora-app/src/history_browser.rs`
- `crates/pinora-app/src/history_export.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `crates/pinora-app/src/lib.rs`
- `crates/pinora-core/src/history.rs`（仅当需补领域删除契约时）
- `AGENTS.md`、`.context/plans/042_history_browser.md`、`.context/tasks/042_history_browser.md`
- `.context/system/overview.md`、`.context/system/risks.md`

## 验收标准

1. 控制窗按 H 显示活动历史；上/下和鼠标选择条目，Esc 关闭。
2. Enter 或重新贴图动作只在文件校验通过后创建新 Pin；失败保持窗口和索引。
3. Delete/Backspace 将选中活动条目先写 tombstone，再执行受管文件清理。
4. 路径穿越、摘要/长度/PNG/尺寸不匹配、缺失文件和索引保存失败均有测试。
5. workspace 质量门禁通过，真实 GUI/桌面验证不夸大。

## 验证

- `cargo fmt --check`
- `cargo test -p pinora-app history_browser::tests -- --nocapture`
- `cargo test -p pinora-app history_export::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：同步 PNG 预览和窗口新增分支会增加 `desktop_shell` 的事件编排复杂度。缓解：面板/解码/删除均保持纯逻辑模块，窗口仅做转换。
- 风险：历史文件在索引外被篡改或删除。缓解：读取前校验长度、摘要、格式、尺寸和受管路径；失败不创建 Pin。
- 回滚：移除 H 窗口入口和历史复用调用，保留既有自动写入与配额清理。

## 完成记录

- 2026-08-02：完成历史面板、受管 PNG 完整性验证、预览、重新贴图和单条 tombstone 删除；控制窗 H 入口和历史窗口事件循环接线完成。
- 2026-08-02：历史复用不重复保存/复制，使用新的 `ImageId` 创建 Pin；删除索引持久化失败恢复原内存索引，清理失败保留 tombstone。
- 验证：`history_browser` 2/2；`history_export` 11/11；`cargo fmt --check`；`cargo check --workspace`；严格 Clippy；`cargo test --workspace`（162 通过，2 忽略）；`git diff --check`；ctx validate，均通过。
- 已知风险：历史预览同步解码且窗口为自绘实验适配器，尚未验证大图卡顿、HiDPI、焦点、读屏和跨平台 GUI；全量清空/搜索/标签/再次编辑未实现。
