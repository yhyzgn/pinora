# 任务 053：贴图渲染缓存与无关重绘隔离

- 状态：进行中
- 计划：`.context/plans/053_pin_render_cache.md`
- 规模：中
- 依赖：`.context/tasks/036_ocr_text_selection.md`、`.context/tasks/047_frame_cache_handoff.md`、`.context/tasks/052_history_render_preprocessing.md`
- 生产行为变更：是；贴图的 OCR、选择、锁定等重绘复用基础帧，避免重复整图缩放和压暗。

## 范围

- 为 `desktop_shell` 当前贴图窗口引入可测试的基础渲染帧缓存。
- 在贴图创建、窗口尺寸、不透明度和缩放改变时精确失效并重建缓存。
- 让 `paint_pin` 只复制缓存并绘制动态叠加层，补充像素与失效契约测试。

## 任务目标

降低常见贴图交互中的主线程像素工作，使拖选 OCR 文本、显示词框、焦点切换和锁定反馈不因重复处理大图而产生明显卡顿。

## 非目标

- 不改变贴图控制键、窗口创建、置顶、KWin、托盘菜单、历史或 OCR 业务功能。
- 不实现真正 alpha 透明、点击穿透、GPU 纹理、后台渲染或帧率基准。

## 预期文件

- `crates/pinora-app/src/desktop_shell.rs`
- `AGENTS.md`
- `.context/plans/053_pin_render_cache.md`
- `.context/tasks/053_pin_render_cache.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 缓存命中时 `paint_pin` 不执行整图 `scale_nearest` 或 `apply_opacity_darken`。
2. resize、缩放和不透明度变更会使缓存产生正确的新基础帧；OCR/边框叠加继续实时显示。
3. 历史重新贴图继续复用 052 的 XRGB，PinTransform 和关闭/隐藏/锁定语义不变。
4. 定向测试、fmt、workspace check、严格 Clippy、全量测试、diff 检查与 ctx validate 通过。

## 验证

- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app pin_window::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：缓存键遗漏导致显示过期尺寸或不透明度。缓解：将缓存身份与像素生成抽成纯函数并覆盖失效场景。
- 风险：贴图峰值内存增加。缓解：只保存当前窗口尺寸的一帧，关闭时连同 `PinWin` 释放，维持现有贴图数量上限。
- 风险：resize 仍需主线程重建。缓解：仅在尺寸改变时重建，动态 OCR/边框重绘不重复全图处理；真实帧率不作未验证声明。
- 回滚：删除缓存字段和辅助函数，恢复原绘制路径；不触及用户图像、索引、配置或领域状态。

## 完成记录

- 2026-08-02：已实现贴图基础帧缓存。命中缓存时 `paint_pin` 只复制帧和绘制 OCR/拖选/边框；窗口尺寸、缩放和不透明度改变时使缓存失效后重建，保持原有缩放后压暗的像素顺序与 `.999` 近不透明边界语义。
- 已新增缓存的缩放、压暗、身份匹配与近不透明像素测试；本地通过 `desktop_shell::overlay_scale_tests`、`pin_window::tests`、fmt、workspace check、严格 Clippy、全量 workspace 测试（app 137 通过、2 个真实桌面测试忽略；core 55 通过）与 `git diff --check`。
- ctx validate 与 GitHub 三平台 CI 尚待本次提交后执行，因此任务仍为进行中；未将离线缓存测试描述为真实 GUI 帧率或跨平台窗口性能验证。
