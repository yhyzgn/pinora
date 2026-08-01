# 任务 016：完成 Grok 实现的生产级接管审计

- 状态：已完成
- 计划：`.context/plans/016_takeover_audit.md`
- 规模：大
- 依赖：`.context/tasks/015_overlay_toolbar.md`
- 生产行为变更：无

## 任务目标

对当前 workspace 的实现、依赖、平台边界、事件循环、外部进程、测试和质量门禁进行证据审计，形成后续重写的优先级与保留/重做决策。

## 范围

- 审查 `src/main.rs`、`pinora-core`、`pinora-app` 和历史提交中的功能演进。
- 运行格式、编译、Clippy、workspace 测试、Windows target 交叉检查和上下文校验。
- 识别架构单体化、Linux 绑定、后台生命周期、错误处理、测试覆盖和可发布性风险。
- 更新 `.context/system/overview.md`、`.context/system/risks.md` 和本计划/任务。

## 非目标

- 不在本任务中删除或重写生产模块。
- 不引入新框架、截图后端、OCR 引擎或平台 SDK。
- 不以 cargo test 通过推断 GUI 或跨平台正确性。

## 预期文件

- 更新 `AGENTS.md` 当前计划/任务指针。
- 新增 `.context/plans/016_takeover_audit.md`。
- 新增并持续更新 `.context/tasks/016_takeover_audit.md`。
- 更新 `.context/system/overview.md` 和 `.context/system/risks.md`。
- 后续重写前可另建审计报告；本任务不修改 `src/` 或 `crates/`。

## 验收标准

- 构建和质量门禁结果有实际命令输出，不只写预期结果。
- 至少确认以下问题并引用源码：2461 行单体桌面壳、Unix/Linux/KDE 硬绑定、无平台条件编译、外部子进程生命周期、GUI/真实桌面测试缺口。
- 明确分类：领域模型和纯几何可优先保留；桌面壳、平台捕获选择、OCR/剪贴板进程封装需迁移或重做；未验证能力不得宣称生产支持。
- 为下一任务提出一个单一、可验证、可回滚的重建目标。

## 审计结论与处置矩阵

| 区域 | 证据 | 处置 | 理由 |
| --- | --- | --- | --- |
| `pinora-core` 几何、图像缓冲、选择约束、领域状态 | `crates/pinora-core/src/{geometry,image,selection,state}.rs`；35 个 core 测试通过 | 保留并加强 | 纯数据不变量和 fake 可测试性是当前少数稳定边界 |
| `pinora-core::annotate` 数据模型 | `crates/pinora-core/src/annotate.rs` 910 行；Clippy 报告函数参数和算术问题 | 保留概念，迁移渲染/事务实现 | 数据模型可复用，但手工像素渲染与编辑状态需要独立测试和性能边界 |
| `pinora-app::desktop_shell` | 2461 行，混合事件循环、平台、绘制、业务、OCR、IPC | 重做主编排边界 | 单体化使竞态、平台差异和 UI 回归不可隔离 |
| `capture_select`、`capture_kde`、`capture_xcap` | KDE Spectacle → xcap → fake；最大屏选择；KDE CLI/kscreen 解析 | 重做平台能力层 | 当前选择器不是跨平台策略，fake 不能作为生产成功回退 |
| `frame_cache` | `frame_cache.rs:39-165` 持续抓屏，`desktop_shell.rs:531-549` 可消费 2 秒内或任意旧帧 | 重做快照服务 | 贴图/控制窗可能被包含进旧帧，且没有显示器/来源世代校验 |
| `ocr.rs` | tesseract CLI、临时 PNG、线程等待、外部 `kill -9` | 重做任务与引擎适配 | 缺取消、generation、可靠清理和跨平台进程管理 |
| `image_sink.rs` | 系统剪贴板命令同步等待；系统失败仍返回 `Ok(())` | 重做导出/剪贴板端口 | UI 会阻塞，事件语义把“内存副本”误报为系统复制成功 |
| `os_instance.rs`、`src/main.rs` | Unix socket、Unix fs2 路径、无 `cfg` 平台模块 | 重做单实例/IPC adapter | Windows target 在依赖阶段已失败，入口还直接绑定 Unix API |
| `region_overlay.rs`、`region_workflow.rs`、独立 `pin_window.rs` | 与 `desktop_shell` 自有 Overlay/贴图路径并存，主要仅由 `lib.rs` 导出 | 迁移后废弃 | 两套交互路径增加维护和行为漂移，不应继续兼容性堆积 |
| 托盘/热键 | `tray.rs`、`hotkey.rs` 直接初始化 GTK/全局后端，失败仅打印 | 迁移到平台能力与诊断 | 缺统一生命周期、状态模型和用户可见反馈 |

## 已验证事实、推断与未知项

### 已验证事实

- 原生 Linux 构建通过；workspace 测试可执行 75 个测试，2 个真实桌面测试被忽略。
- fmt 和零警告 Clippy 不通过；Windows target 检查在 GTK/pkg-config 阶段失败。
- 入口、单实例、KDE 捕获、Linux 剪贴板存在明确 Unix/Linux/KDE 代码路径。

### 推断

- 在当前壳体上继续加功能会继续产生局部修补和回归，重建应用编排边界比继续修补更低风险。
- 领域层的几何与图像不变量比 UI 和平台代码更接近可复用生产基础。

### 未知项

- Windows/macOS 原生截图、透明置顶、托盘和全局热键方案尚未选型和验证。
- Wayland Portal、KDE KWin D-Bus 和 OCR 引擎的正式 API/许可证/分发方式尚未完成官方资料核验。
- 真实多显示器、HiDPI、窗口切换和退出竞态没有自动化桌面探针。

## 验证

- `cargo fmt --check`（当前失败，需记录差异）。
- `cargo check --workspace`（当前成功）。
- `cargo clippy --workspace --all-targets -- -D warnings`（当前失败，需记录 lint）。
- `cargo test --workspace`（当前 75 个可执行测试通过，2 个真实桌面测试忽略）。
- `cargo check --workspace --target x86_64-pc-windows-msvc`（当前因 GTK/pkg-config 失败）。
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`（当前受历史计划/任务契约漂移阻塞）。

## 风险与回滚

- 风险：审计结论可能要求较大范围重写。缓解：只在后续任务按模块边界迁移，保留旧路径直到新路径有等价测试。
- 风险：当前工作区 `AGENTS.md` 含用户/工具追加的 `claude-mem-context` 未提交修改；本任务只在其现有内容上更新指针，不覆盖该段。
- 回滚：恢复本任务新增/修改的上下文文件和指针；不回滚用户已有 `AGENTS.md` 修改，不触碰源码。

## 完成记录

- 状态：已完成（2026-08-01）。
- 已验证：`cargo check --workspace` 成功；`cargo test --workspace` 40 个 app + 35 个 core 测试通过，2 个真实桌面测试忽略；`cargo fmt --check` 失败；Clippy 零警告失败；Windows target 检查在 GTK/pkg-config 失败；`context_bootstrap.py validate` 已在补齐 011–015 历史契约后返回 `ok: true`。
- 已确认：`desktop_shell.rs` 2461 行；项目无平台条件编译；入口/单实例/捕获/剪贴板存在 Linux/KDE 绑定；OCR 与剪贴板通过外部子进程；核心行为尚未有 GUI 端到端覆盖。
- 初步决策：`pinora-core` 的几何、选择、图像不变量和领域状态优先保留并加强测试；桌面壳、平台能力、任务管理和外部命令边界列为迁移/重做对象。
- 阻塞项：历史 `011`–`015` 上下文资产存在契约格式漂移，导致 `ctx validate` 当前失败；在不改变历史事实的前提下需单独修复上下文门禁。
- 下一步：新建一个只负责“平台服务与应用状态机边界”的重建任务，先让 core 和 app 的非 GUI 契约可跨 target 编译，再迁移 Overlay。
