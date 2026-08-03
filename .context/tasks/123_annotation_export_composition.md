# 任务 123：标注导出图像合成契约

- 状态：已完成
- 计划：`.context/plans/123_annotation_export_composition.md`
- 规模：中
- 依赖：任务 114、117、121、122 已完成。
- 生产行为变更：否；内部导出图像合成所有权迁移。

## 任务目标

让 `pinora-export` 唯一拥有 Overlay 导出来源选择和标注合成，令 app 只提供已裁剪图像、标注会话和导出工作流编排。

## 变更前记录

```text
目的：将原图/标注图导出来源与标注合成逻辑迁入 pinora-export，删除 desktop_shell 本地副本。
影响路径：Overlay 复制、保存、贴图、OCR 与贴图复制/保存的图像输入。
兼容性：不改变接口、数据、状态、租户或权限语义；贴图始终使用标注图。
外部副作用：无；纯图像合成不访问文件、剪贴板、窗口、线程或外部基础设施。
回滚点：恢复 desktop_shell 内来源枚举和合成函数，移除 pinora-export 对应导出。
验证场景：原图不变、已提交标注烧录、草稿回退、异常长度回退、贴图来源强制标注。
```

## 范围

- 新增 `crates/pinora-export/src/capture_export.rs`。
- 迁移导出来源枚举、`CaptureImage` 标注合成与草稿回退。
- 切换 app 的工具栏来源状态、Overlay 裁剪和贴图复制/保存路径。
- 迁移并补强对应像素回归测试。
- 更新 crate 导出、设计/系统/风险文档。

## 非目标

- 不改变标注工具、图像裁剪、编码、文件/剪贴板 IO、导出任务监督、Window/Surface、softbuffer、截图、OCR、历史、托盘或 EventLoop。
- 不改变原图/标注图默认值、复制/保存/贴图语义或用户可见像素。

## 预期文件

- `AGENTS.md`
- `.context/plans/123_annotation_export_composition.md`
- `.context/tasks/123_annotation_export_composition.md`
- `crates/pinora-export/src/{lib,capture_export}.rs`
- `crates/pinora-app/src/{lib,desktop_shell}.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. export crate 唯一拥有来源枚举与标注图合成；app 删除本地同类实现。
2. 原图、已提交标注、草稿回退和长度不匹配均由 export 测试覆盖，依赖方向不变。
3. app 仍独占选区、会话、资产身份、任务提交、Window/Surface、softbuffer present 和唯一 EventLoop；贴图保持强制标注来源。

## 验证

- `cargo test -p pinora-export -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：来源映射、文档/草稿优先级或缓冲长度回退变化会造成导出图像像素回归。
- 回滚：恢复 app 内来源枚举/合成/克隆函数，移除 export crate 对应导出；不触碰编码、IO、窗口、输入、OCR、历史、托盘或数据格式。

## 完成记录

- 2026-08-03 已完成。新增 `pinora-export::capture_export`，以
  `CaptureExportSource::{Original, Annotated}` 与
  `compose_capture_export_image` 固定原图隔离、已提交文档优先、仅草稿预览和异常长度
  回退语义；`desktop_shell` 改为使用该公开契约，贴图完成动作仍强制 `Annotated`。

- 验证通过：`cargo test -p pinora-export -- --nocapture`（30 通过，1 项真实剪贴板测试
  忽略）、`cargo test -p pinora-app --lib -- --nocapture`（33 通过）、
  `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`cargo check --workspace`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --check`、
  `git diff --check` 与 `ctx validate`。

- 未覆盖风险：上述离线/交叉编译验证不构成真实剪贴板、文件权限、GUI、任务栏/Dock、
  HiDPI、焦点或性能验收，继续由 R-074 跟踪。
