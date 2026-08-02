# 任务 068：Overlay 标注提交层缓存

- 状态：已完成
- 计划：`.context/plans/068_overlay_preview_cache.md`
- 规模：中
- 依赖：`.context/tasks/030_annotation_revision_contract.md`、`.context/tasks/053_pin_render_cache.md`、`.context/tasks/061_tray_only_window_boundary.md`、`.context/tasks/066_auxiliary_window_visibility_policy.md`、`.context/tasks/067_blur_annotation.md`
- 生产行为变更：是；降低现有 Overlay 连续标注的重复合成开销，不改变可见像素语义。

## 任务目标

在既有 Overlay 中缓存当前源选区的已提交标注层。拖拽草稿只在该层上叠加，而不是每帧重新烧录完整 `AnnotationDoc`；预览、提交、撤销/重做、导出和贴图输出保持确定性一致，且 Pinora 仍只通过 tray 驻留、辅助窗口仍不得进入任务栏/Dock。

## 范围

- 提供核心草稿叠加接口，并把完整预览实现收敛到同一语义。
- 新增 Overlay 生命周期私有的原始选区/已提交 RGBA 缓存及 revision/选区失效键。
- 复用既有 XRGB 显示缓存和 damage present，不改建窗或任务路径。
- 新增核心像素、缓存失效和 `window_policy` 回归，更新上下文事实与风险。

## 非目标

- 不引入 GPU、线程、帧率指标、图像格式、任何新标注工具、对象编辑、持久化或跨会话缓存。
- 不改变截图、热键、托盘、贴图、OCR、导出、IPC 或窗口工厂。

## 预期文件

- `crates/pinora-core/src/{annotate.rs,lib.rs}`
- `crates/pinora-app/src/{lib.rs,desktop_shell.rs,overlay_preview_cache.rs,window_policy.rs}`
- `AGENTS.md`
- `.context/plans/068_overlay_preview_cache.md`
- `.context/tasks/068_overlay_preview_cache.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 草稿叠加接口与完整 `render_preview_rgba` 对图形、文本、马赛克和 Blur 均逐字节一致；源图与已提交层不被修改。
2. Overlay 缓存对同一源选区/revision 的草稿拖拽重用已提交层；提交、撤销、重做、重选或尺寸变更安全失效。
3. 缓存仅属于当前 Overlay，尺寸或源缓冲异常时不 panic、不泄漏旧像素。
4. 不新增建窗、事件循环、截图或 worker；`window_policy` 递归源码守卫持续通过。
5. 定向和全量离线门禁通过；真实 4K/HiDPI 帧时间、tray、任务栏/Dock、焦点和合成器行为仍明确为未验证。

## 验证

- `cargo test -p pinora-core annotate -- --nocapture`
- `cargo test -p pinora-app overlay_preview_cache::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：大选区缓存提高峰值内存。缓解：每个 Overlay 仅持有一份源裁剪和提交层，随重选/关闭释放；真实峰值留为开放风险。
- 风险：缓存键漏掉 revision 或源选区导致陈旧像素。缓解：把 revision/选区放在缓存键内，并为变更路径增加定向测试。
- 风险：性能优化误改变马赛克/Blur 的原图采样顺序。缓解：核心逐字节等价回归，保持草稿像素处理从原始裁剪读取。
- 风险：入口绕过窗口策略。缓解：不增加窗口 API 并运行递归源码守卫。
- 回滚：移除缓存并恢复完整预览路径；标注模型、tray 与窗口策略不变。

## 完成记录

- 已新增 `render_draft_rgba` 并让完整预览走“已提交层 + 草稿叠加”的同一像素语义；图形、文本、马赛克与 Blur 草稿和完整路径逐字节一致，源图和提交层不被草稿修改。
- 已新增 Overlay 生命周期私有 `overlay_preview_cache`：缓存键包含源选区和 `AnnotationRevision`，草稿移动重用已提交层，提交、撤销/重做、重选和无效裁剪安全失效；现有 XRGB/damage present 保持不变。
- 不新增窗口、事件循环、截图或后台任务，`window_policy` 递归守卫持续通过，因此本任务没有扩大任务栏/Dock 或 tray 行为表面。
- 2026-08-02 验证通过：`cargo test -p pinora-core annotate -- --nocapture`、`cargo test -p pinora-app overlay_preview_cache::tests -- --nocapture`、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`、`cargo test -p pinora-app window_policy::tests -- --nocapture`、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check`、`ctx validate`。
- 未验证：真实 4K/HiDPI 帧时间和峰值内存，以及各原生窗口管理器的 tray、任务栏/Dock、焦点和合成器表现；未将这些作为完成证据。
