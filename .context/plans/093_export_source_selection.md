# 计划 093：导出源选择

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/093_export_source_selection.md`

## 目标

在既有 Overlay 内为图片复制和文件保存增加显式的“原图 / 标注合成图”来源选择；贴图的复制和保存继续输出该贴图当前持有的图像内容。所有输出在提交给既有受监督 worker 前冻结，不引入新窗口、截屏或主线程编码。

## 非目标

- 不新增系统级取窗、桌面合成截图或“截图贴图窗口”的实现；贴图的缩放、透明度、OCR 词框、锁定边框和客户区菜单不是图像内容，不能被错误地烧录进导出文件。
- 不持久化 Overlay 本次会话的来源选择，不改变设置 schema、历史格式、领域状态、`PinId`、公共命令或权限。
- 不改变贴图创建、OCR、文本复制、文件格式/质量、命名、PNG-only 历史或现有导出 worker 的语义。
- 不新增窗口、事件循环、后台 worker 类型、外部进程、网络访问或真实桌面 E2E 声明。

## 依赖关系

- 依赖 092 已冻结的 `ExportJobInput::SaveImage`、格式/质量、原子文件发布和 PNG-only 历史边界。
- 依赖 Overlay 的 `crop_overlay_image`、`AnnotationDoc` 与 asset/generation 所有权门禁。
- 依赖现有贴图 `PinWin.image`：它是贴图当前可导出的图像内容，不等价于窗口合成结果。

## 约束

- 来源选择仅在当前 Overlay 的 Ready 阶段生效；默认“标注合成图”，以保持现有复制/保存行为不变；切换仅影响本次 Overlay 会话。
- Overlay 的“原图”是当前源选区裁剪后的未标注 RGBA 像素；“标注合成图”在提交前先提交草稿，再烧录已提交标注。两者都保留既有选区派生 identity 与 worker 所有权门禁。
- Overlay 的贴图动作始终采用标注合成图；OCR 也继续采用标注合成图。来源切换不得意外改变这两条既有路径。
- 已有贴图的复制/保存固定导出 `PinWin.image` 当前像素内容；不得截取或烧录缩放、透明度、OCR 词框、菜单、窗口边框或桌面背景。
- 工具栏来源控件必须在现有 Overlay 客户区内自绘并可见反映当前选择；不得创建对话框、子窗口或任务栏/Dock/分页器入口。

## 阶段

1. 建立会话内来源模型和纯决策函数，锁定默认值、循环切换及 Copy/Save 与 Pin/OCR 的不同语义。
2. 扩展既有自绘 Overlay 工具栏和图像裁剪路径，使 Copy/Save 在提交 job 前选择并冻结正确 RGBA 帧。
3. 覆盖来源切换、原图与合成像素差异、贴图/OCR 固定合成和贴图当前图像输出契约。
4. 执行定向、workspace、跨 target、严格静态、差异和上下文门禁；将真实高 DPI、帧时间和窗口管理器行为如实记录为风险。

## 检查点

1. 新 Overlay 默认复制/保存结果与 092 前的标注合成结果一致；切换后 Copy/Save 只输出原图选区，且不会泄漏草稿或已提交标注。
2. 贴图、OCR 与编辑完成仍始终使用合成图；切换来源不改变 `PinId`、asset/generation、关闭/恢复、窗口策略或 worker 生命周期。
3. 贴图 Copy/Save 只使用当前 `PinWin.image`，不依赖窗口捕获，也不包含视觉缩放、透明度、OCR 叠加或客户区 UI。
4. 来源控件不新建窗口，原有 tray-only 源码守卫和全量回归继续通过。

## 计划级风险

- 大选区在 Overlay 结束时仍需同步生成一次 RGBA 帧；来源选择不增加额外持久缓存或 worker，但实际高分辨率帧时间须在原生会话验证。
- 自绘来源状态在高 DPI 或极窄选区下的可发现性，以及 Windows/macOS/X11/KDE Wayland 的任务栏/Dock/分页器隔离，不能由离线测试证明。

## 变更前记录

```text
目的：补齐导出前选择原图或标注合成图，同时明确贴图导出“当前视图”的像素边界。
影响路径：Overlay 会话状态、Overlay 工具栏、自绘状态、裁剪/提交、贴图复制/保存契约、上下文与风险。
兼容性：默认仍为标注合成图；文件格式/质量、命名、剪贴板 PNG、历史、PinId、设置、状态字符串、权限和窗口策略不变。
外部副作用：用户触发复制或保存时继续使用既有受监督本地剪贴板/文件 worker；不联网、不新增窗口、不请求权限。
回滚点：删除来源枚举、工具栏控件和选择分支，恢复 Copy/Save 一律合成图；贴图、OCR 与已有导出路径不变。
验证场景：默认值、循环、原图/合成像素、草稿提交、贴图/OCR 固定合成、贴图当前像素、worker 冻结、窗口策略和 workspace 门禁。
```

## 完成标准

- Overlay 可在复制或保存前明确选择原图或标注合成图，默认行为兼容既有合成输出。
- 贴图与 OCR 语义不被来源控件改变，贴图 Copy/Save 精确输出其当前图像内容。
- 定向、workspace、跨 target、严格静态、差异和 `ctx validate` 通过；真实性能、高 DPI 和桌面窗口管理器行为保留为未覆盖风险。

## 完成记录

已完成。

- Overlay 新增仅会话内的 `OverlayExportSource::{Original, Annotated}`，默认 `Annotated`；工具栏在既有客户区显示并切换 `RAW`/`ANN`。该状态不写入设置、领域、历史或日志内容。
- `OverlayFinish::Copy` 与 `Save` 在结束 Overlay、提交既有 `ExportJobService` 前冻结所选 RGBA 帧：原图不访问标注文档或草稿，合成图先提交草稿并烧录文档。`Pin` 与 OCR 明确固定合成图，保留既有行为。
- 贴图 Copy/Save 通过 `pin_export_image` 仅克隆当前 `PinWin.image`；没有窗口/桌面截图，因此缩放、透明度、OCR 词框、锁定边框和客户区菜单不会进入输出。
- 为工具栏状态、来源循环/动作路由、原图与合成像素差异、贴图当前像素输出补充回归测试；为工具栏瞬态状态引入 `ToolbarPaintState`，消除严格 Clippy 的参数过多告警，未使用任何告警抑制。
- 已验证：`cargo test -p pinora-app overlay_toolbar -- --nocapture`（8 通过）；`cargo test -p pinora-app desktop_shell -- --nocapture`（36 通过）；`cargo fmt --all -- --check`；`cargo check --workspace`；`cargo clippy --workspace --all-targets -- -D warnings`；`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 272 通过、2 忽略；core 89 通过）；`cargo check --workspace --target x86_64-pc-windows-msvc`；`git diff --check`；`ctx validate` 均通过。
- 未覆盖风险：真实 Windows/macOS/Linux X11/KDE Wayland 的 RAW/ANN 可发现性、HiDPI、结束帧时间、系统剪贴板/文件结果、焦点及任务栏/Dock/分页器隔离尚未验证，见 `R-051`。
