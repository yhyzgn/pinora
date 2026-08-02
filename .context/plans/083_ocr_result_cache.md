# 计划 083：OCR 结果缓存与重复识别消除

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/083_ocr_result_cache.md`

## 目标

让同一运行期内、同一资产版本且同一 OCR 语言预设的重复识别不再重新启动 `tesseract`。缓存命中必须只复用已经通过 `JobSupervisor` owner、终态和 `AssetRef` generation 门禁的结果；命中后仍执行既有文字层刷新与文本复制路径。

## 非目标

- 不持久化 OCR 正文、词框、图像指纹或模型输出；进程退出后缓存必须消失。
- 不新增任意内容哈希、模型下载、OCR 并发合并、跨进程缓存、重试 UI、置信度筛选、云端 OCR 或设置 schema 字段。
- 不修改 Tesseract 参数、语言模型选择、worker 取消/超时、历史格式、导出格式、截图、热键或 tray-only 窗口策略。

## 依赖关系

- 依赖 082 的冻结 `OcrLanguage`、`OcrJobService`、`JobSupervisor` 和本地 Tesseract 适配器。
- 依赖 `AssetRef` 的图像 ID/generation 契约以及 `desktop_shell` 现有 OCR 完成处理。
- 后续如增加引擎、模型目录或阈值，必须把它们作为缓存键的独立配置摘要任务处理。

## 约束

- 缓存键必须至少包含完整 `AssetRef` 和冻结 `OcrLanguage`；不同 generation 或语言不得命中。
- 只有 accepted 成功结果可进入缓存；失败、超时、取消、关闭 owner、陈旧 asset 和未完成 worker 一律不能缓存。
- 缓存必须有固定条目数与估算字节上限；过大结果不缓存，命中只返回克隆，调用方不能修改内部条目。
- 命中路径不得创建 OCR worker、Tesseract 子进程、窗口或事件循环；仍复用现有受监督文本复制和 tray 反馈。

## 阶段

1. 在 `OcrJobService` 定义受限内存 cache，在 accepted completion 后写入，并以 asset/language 查询。
2. 将 `desktop_shell` 的 OCR 成功交付收敛为共享路径，在新任务提交前查询缓存，命中时直接交付。
3. 覆盖命中、语言/代际隔离、失败不缓存、容量淘汰与既有 worker 监督回归。

## 检查点

1. 同一 asset + English 可命中；Auto 或下一 generation 必须 miss。
2. worker 失败、超时、取消、owner 关闭或 generation 失效后查询不得得到结果。
3. 重复 OCR 命中不会启动新的 worker，且贴图文字层、全文复制和 tray 状态沿用成功交付路径。

## 完成标准

- 缓存策略和 `OcrJobService` 均有离线测试，证明 key 隔离、成功写入、失败拒绝、容量边界和克隆隔离。
- `desktop_shell` 只保留一条 accepted/cache 命中的 OCR 结果交付实现；无新窗口、网络或后台重复 OCR。
- 定向、workspace、Windows target、严格 Clippy、差异和 ctx 门禁通过；真实连续点击、内存峰值、模型变更和桌面交互如实记录为未覆盖风险。

## 计划级风险

- OCR 全文和词框可能很大；没有字节/条目上限会让常驻 tray 进程持续增长。
- 内存缓存会在已运行期间保留此前模型得到的结果；键只代表当前可配置的语言和 asset，不得声称可感知外部模型文件变化。

## 变更前记录

```text
目的：消除同一资产版本和语言预设重复触发 OCR 时不必要的本地子进程等待。
影响路径：OcrJobService、desktop_shell OCR 提交/完成分支、纯逻辑测试、上下文和风险。
兼容性：不修改持久化数据、公共命令、语言 wire、OCR 结果形状、截图、导出、租户或权限语义。
外部副作用：缓存命中不启动 Tesseract；未命中仍沿用既有本机子进程与文本复制行为；无网络和模型下载。
回滚点：移除 cache 查询与条目存储，恢复每次都通过既有 OcrJobService worker 提交。
验证场景：同 asset/language 命中、语言/代际 miss、失败/取消/陈旧拒绝、容量淘汰、命中交付与质量门禁。
```

## 完成记录

- `OcrJobService` 现在持有进程内 LRU 风格结果缓存，键为完整 `AssetRef` 和冻结 `OcrLanguage`。只有 `JobSupervisor::accept_result` 接受的成功结果写入；失败、超时、取消、owner 关闭和 generation 失效均不会写入。
- 缓存最多保留 8 条，累计估算不超过 2 MiB，单条估算超过 512 KiB 不缓存；读取总是返回 `OcrResult` 克隆，外部修改不影响缓存。缓存不持久化，service 销毁即释放。
- `start_or_reuse` 在服务边界先校验 `JobKind::Ocr`，命中时不创建 worker；桌面壳把缓存命中和 worker accepted completion 统一送入同一 OCR 交付函数，保留词框刷新、全文复制和 tray 反馈。
- 已验证：服务命中/隔离/失败拒绝/取消/容量/克隆测试、Overlay 回归、workspace 离线测试、格式、workspace 编译和严格 Clippy。Windows target、ctx 和真实桌面/内存压力验证在本任务最终门禁中继续执行或记录。
