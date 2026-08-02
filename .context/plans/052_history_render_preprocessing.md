# 计划 052：历史图像后台渲染预处理

- 状态：进行中
- 负责人：Codex
- 当前任务：`.context/tasks/052_history_render_preprocessing.md`

## 目标

将历史图像读取完成后的 RGBA 到 XRGB、暗化底图和历史编辑 Overlay 预览帧准备移入既有单 worker。历史预览、重新贴图和再次编辑的主线程只消费已按意图准备的不可变结果，避免在大图完成时执行整图像素循环而阻塞输入或窗口绘制。

## 非目标

- 不修改历史索引、受管 PNG、摘要校验、tombstone、配额、搜索、删除或导出语义。
- 不新增缩略图持久化、列表虚拟化、GPU 渲染、UI 框架、窗口或外部依赖。
- 不把离线 worker 测试或 CI 说明为真实慢盘、帧率、HiDPI、读屏或跨平台 GUI 验收。

## 依赖关系

- 依赖 051 的单 worker、取消、截止时间、条目 generation 与当前选择门禁。
- 复用 047 的 `rgba_to_xrgb_and_dim` 纯像素转换，不改变屏幕捕获帧缓存路径。
- 依赖 050 的窗口策略；本任务不创建或改变任何窗口，辅助窗口继续仅经统一策略创建。

## 约束

- worker 按明确意图准备：预览只输出 XRGB 预览像素；重新贴图输出原图与 XRGB 像素；再次编辑输出原图、XRGB 基底与暗化底图。
- 预览不得把完整 RGBA 原图送回 UI 线程；重新贴图与编辑只在当前 `JobId`、owner、条目 generation 和当前选择均匹配时消费。
- RGBA 到 XRGB/暗化处理前后均检查取消；取消、超时或陈旧结果不得修改面板、创建贴图或打开 Overlay。
- worker 不持有窗口、softbuffer、托盘、剪贴板或事件循环句柄；主线程仍是窗口创建、呈现和帧缓存暂停的唯一所有者。
- 新增像素缓冲只能按当前意图生成，不能为预览无条件分配编辑所需的暗化整图。

## 检查点

- `HistoryLoadJobService` 的 worker 在读取/解码后完成对应意图的预处理；桌面壳不会在历史完成分支调用全图 RGBA 转换。
- `HistoryWindow` 缓存已准备 XRGB 预览；历史重新贴图直接复用 worker 输出的 XRGB；历史编辑直接复用 worker 输出的 base/dimmed。
- 成功、失败、取消、陈旧和关闭路径保持 051 的结果门禁与帧缓存恢复语义。
- 纯转换、服务契约与 desktop shell 回归在严格 workspace 门禁下通过。

## 计划级风险

- 大图在 worker 内仍会占用 CPU/内存；单 worker 和按意图输出限制并发与无用分配，但不承诺消除所有计算成本。
- 重新贴图和编辑的结果类型会扩大穷尽匹配面；必须由契约测试和严格 Clippy 覆盖。
- 离线测试无法度量真实窗口重绘、显示器缩放或输入延迟；这些桌面性能证据继续开放。

## 阶段

1. 定义按加载意图区分的后台结果，并补像素准备与取消契约测试。
2. 让预览、重新贴图和编辑分别消费相匹配的准备结果，消除完成交付时的整图转换。
3. 执行定向、workspace 和 CI 门禁，记录真实桌面性能缺口。

## 变更前记录

```text
目的：移除历史加载完成后仍在 UI 线程执行的整图 RGBA 转换，降低大图切换、贴图和再次编辑的卡顿风险。
影响路径：history_load_job、history_window、desktop_shell、上下文文档。
兼容性：不改变公共 IPC、历史索引、PNG、状态字符串、权限、租户或任务门禁语义。
外部副作用：只读取既有受管本地 PNG；不连接外部服务、不创建新窗口、不重新截图。
回滚点：恢复完成交付后的 UI 线程像素准备；历史索引、文件与用户数据不受影响。
验证场景：三类意图的准备结果、取消/陈旧拒绝、预览不回传 RGBA、历史贴图/编辑回归、严格质量门禁。
```

## 完成标准

- 历史预览、重新贴图和再次编辑的全图 XRGB/暗化转换在历史 worker 内完成。
- UI 完成分支仅匹配并消费准备结果，取消或陈旧结果不产生 UI 副作用。
- 定向测试、workspace 严格门禁、ctx validate 和对应 GitHub CI 通过；真实桌面性能缺口如实记录。

## 完成记录

- 2026-08-02：`HistoryLoadJobService` 新增按意图的 `Preview`、`Pin`、`Editor` 负载；worker 在摘要校验与 PNG 解码后生成相应 XRGB/base/dimmed 像素。预览负载仅传递尺寸与 XRGB，不向 UI 回传 RGBA 原图。
- 历史预览缓存、重新贴图和再次编辑分别复用 worker 输出；历史完成分支不再执行这些整图转换。贴图接收预处理像素时验证其长度与图像尺寸一致，不匹配时明确失败而不退回 UI 线程重算。
- 本地验证：`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check` 与 ctx validate 通过（app 135 通过、2 个真实桌面测试忽略；core 55 通过）。GitHub 三平台 CI 尚未运行，本任务保持进行中；真实慢盘、窗口重绘、HiDPI、读屏和输入延迟探针未覆盖。
