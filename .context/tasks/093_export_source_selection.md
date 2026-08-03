# 任务 093：导出源选择

- 状态：已完成
- 计划：`.context/plans/093_export_source_selection.md`
- 规模：中
- 依赖：092 多格式受监督导出、Overlay 标注合成、贴图 `PinWin.image` 与既有 tray-only 窗口策略。
- 生产行为变更：是；Overlay 的图片复制和文件保存可选择原图或标注合成图，默认保持标注合成图。

## 任务目标

把设计文档要求的导出源选择落到现有图像管线，而不以“当前贴图视图”为名截取窗口或混入显示层 UI。

## 范围

- 定义仅当前 Overlay 会话使用的来源模型：原图与标注合成图，默认标注合成图。
- 在现有 Overlay 自绘工具栏加入来源切换及当前状态反馈；Copy/Save 在结束 Overlay、提交 export job 前按当前来源生成并冻结 `CaptureImage`。
- 固定 Pin 与 OCR 为合成图路径；明确 Pin Copy/Save 只提交 `PinWin.image` 当前像素内容。
- 补充来源选择、像素、动作路由和贴图输出的回归测试；更新工作指针、系统全景和风险。

## 预期文件

- `crates/pinora-app/src/{desktop_shell.rs,overlay_toolbar.rs,pin_context_menu.rs}`（仅实际需要的文件）
- `AGENTS.md`
- `.context/plans/093_export_source_selection.md`
- `.context/tasks/093_export_source_selection.md`
- `.context/system/{overview.md,risks.md}`

## 非目标

- 不更改 export job、图像编码、设置 schema、命名模板、历史读写、clipboard 格式、领域接口或单实例协议。
- 不创建“导出选项”新窗口、文件选择器、窗口截图、屏幕截图、进度 UI 或外部服务。
- 不把离线像素测试、交叉编译或 CI 表述为真实 GUI 流畅度、任务栏/Dock/分页器或跨平台桌面验收。

## 验收标准

1. Overlay 默认标注合成图；来源控件可循环切换，Copy/Save 按选择冻结原图或合成 RGBA，并在任务运行期间不再读取 UI 状态。
2. 原图不含草稿或已提交标注；合成图在 Copy/Save 前提交草稿并含完整标注。贴图和 OCR 始终使用合成图。
3. Pin Copy/Save 继续使用 `PinWin.image`，不抓取窗口表面，且不改变现有 owner/generation、格式/质量、历史或错误反馈。
4. Overlay 来源控件只在当前窗口自绘，不新增窗口或展示入口；现有窗口策略和 workspace 回归不破坏。

## 验证

- `cargo test -p pinora-app overlay_toolbar -- --nocapture`
- `cargo test -p pinora-app desktop_shell -- --nocapture`
- `cargo test -p pinora-app pin_context_menu -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：大图在结束 Overlay 时同步产出原图或合成图，实际帧时间仍与分辨率、标注复杂度和平台呈现有关。缓解：仅选择一条现有输出路径、复用现有 worker、不增加窗口捕获或缓存；保留原生会话性能验证。
- 风险：来源状态在高 DPI 或窄选区下不够清晰，或现有辅助窗口在原生桌面仍出现任务栏/Dock/分页器。缓解：自绘控件留在既有窗口，运行既有源码守卫；平台验收单独进行。
- 风险：来源选择影响 Pin 或 OCR。缓解：用纯动作路由锁定两者始终合成，并保持 Pin 直接复制当前图像。
- 回滚：删除来源状态与工具栏动作，使 Overlay Copy/Save 再次固定合成图；导出 worker、Pin、OCR、历史、格式、窗口策略和设置均不变。

## 完成记录

已完成。

- 既有 Overlay 工具栏新增 `CycleExportSource`，自绘状态为 `ANN`（默认标注合成图）或 `RAW`（原图）；点击只更新当前 Overlay 内存状态并请求同一窗口重绘，没有新窗口、事件循环或展示入口。
- `finish_overlay_action` 在提交草稿后将 Copy/Save 映射为当前来源，将 Pin 固定映射为合成来源；`overlay_ocr` 也显式请求合成来源。所选图像在已有 owner/generation worker 提交前生成，后续 UI 状态不能改变已提交任务。
- `render_overlay_export_image` 以纯函数锁定原图与合成图像素边界；`copy_pin_image`、`save_pin_image` 经 `pin_export_image` 输出当前 `PinWin.image`，明确不读取窗口表面。
- `ToolbarPaintState` 将工具栏渲染的活动工具、颜色、填充和导出来源收敛为一个值对象，修复严格 Clippy `too_many_arguments`，未引入任何 lint suppression。
- 验证：`cargo test -p pinora-app overlay_toolbar -- --nocapture`（8 通过）；`cargo test -p pinora-app desktop_shell -- --nocapture`（36 通过）；`cargo fmt --all -- --check`；`cargo check --workspace`；`cargo clippy --workspace --all-targets -- -D warnings`；`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 272 通过、2 忽略；core 89 通过）；`cargo check --workspace --target x86_64-pc-windows-msvc`；`git diff --check`；`ctx validate` 均通过。
- 已知风险：离线测试不能证明真实高 DPI 下的 `RAW`/`ANN` 可读性、复杂标注结束时的帧时间、系统剪贴板/文件结果、焦点或任务栏/Dock/分页器隔离；详情见 `R-051`。
