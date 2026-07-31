# 计划 014：贴图 OCR（本地 Tesseract）

## 目标

对当前贴图图像做本地 OCR，复制全文到剪贴板，并在贴图上叠加词框预览。

## 交付

1. `pinora-core`：`OcrResult` / `OcrWord` / `OcrLine` 模型与全文拼接
2. `pinora-app`：`tesseract` CLI 引擎（按需、缺依赖可降级）
3. 贴图键 `O` 触发识别；`T` 切换词框显示；文本剪贴板 `wl-copy`/`xclip`
4. 无 `tesseract` 时给出明确提示，不崩溃

## 非目标

- 交互式拖选跨行编辑器
- 自动下载语言模型
- OCR 全局热键 / 设置页

## 依赖（运行时，非编译）

- `tesseract` CLI
- tessdata：`eng`（可选 `chi_sim`）

```bash
# Fedora 示例
sudo dnf install tesseract tesseract-langpack-eng tesseract-langpack-chi_sim
```
