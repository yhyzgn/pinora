# 任务 049：历史图像再次编辑

- 状态：进行中
- 计划：`.context/plans/049_history_editing.md`
- 规模：中
- 依赖：`.context/tasks/042_history_browser.md`、`.context/tasks/047_frame_cache_handoff.md`、`.context/tasks/048_full_display_capture.md`、`.context/tasks/050_tray_only_windows.md`
- 生产行为变更：是；新增历史“编辑”入口，将已验证的历史图像带入现有 Overlay 标注会话。

## 范围

- 为 `HistoryPanel` 增加 `Edit` 动作、`E` 键和可见编辑按钮，保持 Enter/Pin、删除和清空语义不变。
- 在 `desktop_shell` 中安全加载选中历史图像，转换为现有 `PreparedPreview`，并打开全图已确认的编辑 Overlay。
- 将“屏幕捕获方式”“初始选区”“窗口呈现”拆成独立内部概念，防止历史编辑被误标为全显示器捕获。
- 增加面板与 Overlay 目标/初始选区纯逻辑测试。

## 任务目标

让用户可在不重新截图、不复制大图像缓冲、不中断既有历史文件安全边界的前提下，再次标注一条历史图像。

## 非目标

- 不修改历史文件或历史条目的身份；新的导出仍沿现有保存历史规则创建独立条目。
- 不将历史编辑窗口宣称为已完成真实跨平台、窗口定位、缩放、无障碍或多显示器验收。

## 预期文件

- `crates/pinora-app/src/history_browser.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `AGENTS.md`
- `.context/plans/049_history_editing.md`
- `.context/tasks/049_history_editing.md`
- `.context/system/overview.md`
- `.context/system/risks.md`

## 验收标准

1. 选中历史条目时，`E` 和编辑按钮产生 `HistoryPanelAction::Edit`；Enter 和 Pin 按钮仍产生 `Reopen`。
2. 编辑仅在 `load_history_image` 成功后打开 Overlay；完整初始选区覆盖原图，且使用新会话身份。
3. 历史加载或 Overlay 创建失败会保留历史窗口、标记错误并恢复帧缓存；不得创建 Pin 或新历史条目。
4. workspace fmt/check/Clippy/test、diff 检查和 ctx validate 通过。

## 验证

- `cargo test -p pinora-app history_browser::tests -- --nocapture`
- `cargo test -p pinora-app history_export::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：历史图像的原显示器已不存在，若强制恢复全屏坐标会导致不可达窗口；使用独立编辑窗口，输出元数据仍保留原始来源坐标。
- 风险：历史加载或窗口创建失败后帧缓存一直暂停；在打开失败分支显式恢复，并由纯逻辑辅助函数锁定意图。
- 风险：将历史编辑偷换为全屏截图使日志和权限语义错误；分离初始化选择和窗口呈现类型。
- 回滚：删除 Edit 动作与编辑入口；历史预览、Pin、删除和索引数据不受影响。

## 完成记录

- 2026-08-02：已实现 `Edit` 按钮与 `E` 动作，复用 `load_history_image` 的受管路径、文件类型、长度、摘要、PNG RGBA8、尺寸和元数据校验。
- 已将屏幕捕获方式、初始选区和窗口呈现拆为独立内部概念；历史编辑为普通窗口 `HistoryEditor` 呈现、全图选区、最小边长 1，不尝试恢复旧显示器的全屏位置。
- 本地通过 `history_browser`、`desktop_shell::overlay_scale_tests`、`fmt`、workspace check、严格 Clippy 与全量测试；待本提交的 GitHub CI。真实 GUI 失败分支、HiDPI 和多屏验收仍未完成。
