# 计划 068：Overlay 标注提交层缓存

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/068_overlay_preview_cache.md`

## 目标

修复当前 Overlay 标注拖拽每帧从头烧录全部已提交文档的性能边界。选区和已提交 revision 稳定时缓存不可变提交层；草稿拖拽仅从该层生成预览，马赛克与模糊仍从对应选区的原始不可变截图采样。该优化不得改变最终合成字节、资产 generation、撤销重做、导出、贴图、tray 或任务栏/Dock 行为。

## 非目标

- 不增加 GPU、线程池、后台渲染、帧率承诺、模糊核、工具、持久化格式或平台窗口 API。
- 不改变截图后端、Overlay 选区、标注事务、导出/OCR 任务监督或 `window_policy`。

## 约束

- `pinora-core` 提供“将当前草稿叠加到既有 RGBA 提交层”的受检验接口；其输出必须与现有 `render_preview_rgba` 逐字节一致。
- `pinora-app` 缓存键必须至少包含源选区和已提交 `AnnotationRevision`；重选、文档 mutation、尺寸异常和缓存失效不得复用旧像素。
- 预览缓存仅在单个 Overlay 生命周期内持有，不跨资产、会话、贴图或窗口复用；不创建窗口、事件循环、任务或截图。
- 必须限制为当前选区一份原始裁剪和一份已提交 RGBA 层；不得在每次草稿事件克隆完整已提交 `AnnotationDoc` 或重新烧录其全部项目。

## 依赖关系

- 依赖 030 的 `AnnotationRevision` 和预览/导出 generation 契约。
- 依赖 053 的显示帧缓存和 067 的原始图像马赛克/Blur 采样语义。
- 依赖 061/066 的 tray-only GUI 会话、隐藏创建与唯一辅助窗口映射入口。

## 阶段

1. 提取核心草稿合成接口并以像素回归锁定与现有预览等价。
2. 建立 Overlay 生命周期私有的提交层缓存，按选区/revision 失效。
3. 将既有预览 XRGB 缓存接到提交层，补性能语义与窗口策略回归并执行全量门禁。

## 检查点

1. 含已提交图形、马赛克或模糊及移动草稿的预览与完整重烤逐字节一致。
2. 草稿移动不触发已提交层重建；提交、撤销、重做或重选恰好重建一次，且不泄漏到新选区。
3. 辅助窗口只经 `window_policy` 创建/映射，递归守卫持续通过。

## 计划级风险

- 单一 Overlay 在大选区内额外持有原始裁剪和提交 RGBA，内存上升；以生命周期绑定、单份缓存和失效清理约束，真实峰值仍待测。
- 离线像素、缓存键和源码守卫无法证明 4K/HiDPI 实际帧时间、任务栏/Dock、焦点或合成器行为；不得将通过测试宣称为桌面性能验收。

## 变更前记录

```text
目的：减少连续标注拖拽时重复烧录完整已提交文档造成的主线程开销。
影响路径：核心草稿合成接口、Overlay 预览缓存、核心/App 测试、上下文记录。
兼容性：不改变标注数据、revision、资产 generation、IPC、持久化、权限或状态字符串。
外部副作用：无网络、外部进程、截图、新窗口或后台任务；仅增加当前 Overlay 生命周期内的内存缓存。
回滚点：移除提交层缓存并恢复完整预览路径；核心像素接口与现有标注数据保持兼容。
验证场景：草稿与完整预览等价、源选区/revision 失效、马赛克/模糊源像素语义、窗口策略守卫。
```

## 完成标准

- 已提交层按选区/revision 缓存；草稿预览不重新烧录完整历史文档，且输出逐字节等价于完整路径。
- 缓存不会跨选区、revision 或 Overlay 生命周期复用；所有无效路径安全回退，未新增窗口或后台任务。
- 定向测试、fmt、workspace check、严格 Clippy、全量离线测试、差异检查和 `ctx validate` 通过；原生性能与桌面窗口验证缺口明确记录。

## 完成记录

- 已在 `pinora-core` 抽取 `render_draft_rgba`，使完整预览先烧录提交文档再叠加当前草稿；对图形、文本、马赛克和 Blur 与完整路径逐字节等价，像素处理仍从不可变原图采样。
- 已新增 `pinora-app::overlay_preview_cache`，仅在源选区或 `AnnotationRevision` 改变时保存原始裁剪与提交 RGBA 层；草稿移动重用该层，重选和无效裁剪显式清理，XRGB/damage present 继续复用既有路径。
- 未新增窗口、事件循环、截图或 worker，辅助窗口仍由 `window_policy` 独占创建和映射。
- 2026-08-02 已通过核心、缓存、Overlay/window policy 定向测试、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check` 与 `ctx validate`。全量离线测试为 app 172 通过、2 个真实桌面用例忽略，core 74 通过。
- 未验证：真实 4K/HiDPI 帧时间、峰值内存，以及 Windows/macOS/X11/KDE Wayland 的 tray、任务栏/Dock、首帧、焦点和合成器行为。
