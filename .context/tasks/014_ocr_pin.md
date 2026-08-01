# 任务 014：贴图 OCR

- 状态：已完成
- 计划：`.context/plans/014_ocr_pin.md`
- 规模：中
- 依赖：`.context/tasks/013_annotate_tools_plus.md`
- 生产行为变更：有

## 任务目标

贴图窗口按 `O` 对当前图像做本地 OCR，全文复制到系统剪贴板，可选词框叠加。

## 范围

- `pinora-core` OCR 模型
- `pinora-app` Tesseract CLI、文本剪贴板、贴图交互
- `.context` 文档与工作指针

## 非目标

- 拖选编辑器、模型下载、设置 UI、OCR 全局热键

## 预期文件

- `crates/pinora-core/src/ocr.rs`。
- `crates/pinora-app/src/ocr.rs`、贴图交互和文本剪贴板适配。
- 对应 `.context` 计划、任务和系统说明。

## 验收标准

- 有 tesseract 时：`O` 得到非空文本并可复制（样例图）
- 无 tesseract 时：错误信息清晰，应用不崩
- `cargo test --workspace` 通过

## 验证

- `cargo test --workspace`
- 单元：TSV 解析、全文拼接
- 可选：安装 tesseract 后手动贴图按 O

## 风险与回滚

- Tesseract CLI、语言模型和系统剪贴板都属于本机外部依赖；失败必须降级并记录。
- 回滚时移除贴图 OCR 入口并保留纯 OCR 数据模型与 TSV 测试，避免影响截图/贴图主路径。

## 完成记录

- 2026-07-31
- `pinora-core`：`OcrWord`/`OcrLine`/`OcrResult`
- `pinora-app`：tesseract CLI + TSV 解析；贴图 `O`/`T`；`copy_text_to_system_clipboard`
- 验证：`cargo test --workspace` 通过；本机无 tesseract CLI 时 `O` 给出安装提示
