# 计划 024：桌面 OCR 接入任务监督

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/024_desktop_ocr_integration.md`

## 目标

将现有 `desktop_shell` 的贴图 OCR 与 Overlay OCR 统一接入 `OcrJobService`：UI 事件只提交任务，事件循环轮询结果；贴图关闭、Overlay 取消、再截或退出会关闭对应 owner，陈旧结果不能更新窗口。

## 非目标

- 不重写 winit/softbuffer 窗口适配器，不拆除旧壳。
- 不增加 OCR 文字编辑器、拖选文本层、缓存、进度 UI 或并发队列策略。
- 不迁移系统剪贴板命令到任务监督器；本轮只保持成功 OCR 的既有复制行为。

## 约束

- `PinId` 作为贴图 owner，Overlay 使用独立 `SessionId`；窗口句柄不得进入任务元数据。
- 每次 OCR 冻结 `AssetRef::initial(image.id)`；主循环交付前从当前贴图/Overlay 重新取资产引用。
- 结果只能在事件循环中更新 `PinWin::ocr` 或执行既有文本复制；worker 不得触碰 `self.pins`、winit 或剪贴板。
- 所有关闭/替换入口必须取消 owner；服务退出前取消全部任务。

## 依赖关系

- 依赖 021 的 `JobSupervisor`、资产版本和 owner 语义。
- 依赖 022 的可取消 OCR child 适配器。
- 依赖 023 的 `OcrJobService` 结果协议。

## 阶段

1. 为 Overlay/Pin 状态补充领域 owner 与资产引用，接入服务字段和主循环轮询。
2. 改写贴图 `O` 键与 Overlay 工具栏 OCR 入口，删除旧同步调用和裸 OCR 线程。
3. 在关闭、再截、取消和退出路径补 owner 取消，并运行静态/离线回归。

## 检查点

- `desktop_shell.rs` 不再直接调用 `recognize_image` 或为 OCR `thread::spawn`。
- 关闭贴图后晚到结果只产生丢弃诊断，不恢复或更新已销毁窗口。
- 事件循环保持非阻塞；真实 Tesseract 仍由 OCR 适配器管理并可取消。

## 计划级风险

- 当前没有 GUI E2E，无法在无授权桌面环境证明窗口视觉结果；以服务契约和静态扫描锁定逻辑，并保留真实桌面探针缺口。
- Overlay OCR 现有行为只复制全文，不持有文字层；本任务不扩大其产品语义。

## 完成标准

- Pin/Overlay OCR 入口均经 `OcrJobService` 提交与轮询，旧直接调用路径删除。
- owner 关闭/取消/退出会发出取消信号，结果按终态和 generation 门禁处理。
- fmt、check、严格 Clippy、workspace 测试、差异检查和上下文校验通过。

## 完成记录

- 状态：已完成（2026-08-01）。
- 实际变更：`desktop_shell` 为贴图绑定 `AssetRef`、为 Overlay 绑定 `SessionId` 与 OCR 资产，持有并轮询 `OcrJobService`；贴图 `O` 键和 Overlay 工具栏 OCR 均改为提交受监督任务。关闭贴图、取消/再截 Overlay 和退出均关闭 owner 或取消全部任务；结果只有在 owner、资产 generation 和任务终态仍匹配时才更新词框或执行既有文本复制。
- 验证：`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（app 60 项通过、core 39 项通过，2 个真实桌面测试忽略）、`rg` 静态扫描、`git diff --check` 与上下文校验通过。
- 残留风险：尚无 GUI E2E；退出路径目前发出取消信号但未等待所有 worker 完成；剪贴板/导出任务仍未纳入统一监督；复杂 OCR 子孙进程回收策略仍需独立验证。
