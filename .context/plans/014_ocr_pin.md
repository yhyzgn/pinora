# 计划 014：贴图 OCR（本地 Tesseract）

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/014_ocr_pin.md`

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

## 约束

- 默认只运行本地 OCR，不自动上传或下载模型。
- 缺少 CLI 或语言模型必须给出错误，不得伪造识别结果。

## 依赖关系

- 依赖 `.context/tasks/013_annotate_tools_plus.md` 的贴图与文字绘制基础。

## 阶段

1. 定义可离线测试的 OCR 结果模型。
2. 接入 Tesseract TSV 解析与本地命令。
3. 接入贴图触发、词框和文本剪贴板。

## 检查点

- TSV 解析和全文拼接有纯单元测试。
- 真正调用 Tesseract 的测试须明确标为本机/可选验证。

## 计划级风险

- 外部子进程、临时文件和模型路径需要生命周期与隐私控制。
- OCR 词框不等同于可交互的跨行文字选择。

## 完成标准

- 本地 OCR 的当期结果模型、CLI 路径和降级说明已记录，不扩大为完整 OCR 编辑器。

## 运行时依赖（非编译）

- `tesseract` CLI
- tessdata：`eng`（可选 `chi_sim`）

```bash
# Fedora 示例
sudo dnf install tesseract tesseract-langpack-eng tesseract-langpack-chi_sim
```
