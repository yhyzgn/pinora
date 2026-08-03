# 系统全景：pinora

## 技术与运行基线

- Rust 2024 workspace：`pinora`（`src/main.rs`）+ `pinora-core` + `pinora-app`。
- 依赖：`ctrlc`、`fs2`、`png`、`image`（仅 JPEG/WebP 编码特性）、`xcap`、`winit`、`softbuffer`、`fontdue`（标注文本）、`tray-icon`/`gtk`（托盘）。
- Linux xcap 需 `pipewire-devel`、`mesa-libgbm-devel`（**仅 xcap/portal 兜底路径**）。
- **当前截图后端（Linux/KDE 实验路径）**：`kde-spectacle`（KWin，~0.5s）→ `xcap`/portal（慢）→ 受限能力状态；`FakeCaptureProvider` 仅由显式测试/开发注入使用，不能是生产截图成功的降级结果。
- **不要默认 portal**：portal/PipeWire 是通用 Wayland 兜底，不是 Snipaste 级体验。
- **全局热键**：`global-hotkey`（可持久化录制的区域/单显示器全屏主键，默认 F2/F3；兼容 Ctrl+N/Ctrl+Shift+S 区域备用键），注册受桌面环境限制；保留单实例 IPC `pinora capture`，启动时写入 `~/.local/share/applications/pinora.desktop`。
- **系统剪贴板**：Linux 优先 `wl-copy`，回退 `xclip`；同步 `LocalImageSink` 先保留内存副本，系统写入失败返回 `ClipboardFailed` 而不发布成功，适配器直接持有子进程并在截止时间后回收；桌面异步复制仍由 `ExportJobService` 监督，真实读回和跨平台原生后端未验证。

## 当前可运行的实验能力（未达到生产声明）

| 能力 | 说明 |
| --- | --- |
| 截屏尝试 | KDE 优先 spectacle/KWin；否则 xcap；两者不可用时返回 `CapabilityUnavailable`，不生成 fake 图像 |
| 区域与全屏 Overlay | F2/Ctrl+N 拖选；选区实时显示源图物理像素宽高与全局左上坐标；已确认且未标注的选区可拖四边/四角精确调整；F3/托盘默认全屏自动确认当前完整图像；多显示器 tray 可指定目标全屏；双击复制、中键/Enter 贴图；选区内标注/OCR |
| 贴图窗口 | 无边框置顶、拖动、滚轮及四边/四角等比缩放、双击或客户区 `100%` 恢复原图、Esc 关闭；普通 Overlay 新贴图优先在当前捕获范围的右/左/下/上避开来源选区，无空间时稳定回退；tray 可撤销最近关闭一次（恢复为新 PinId）并通过无内容泄露的动态贴图列表唤起既有贴图；客户区右键菜单、锁定/压暗/置顶、原位编辑；菜单 `PASS` 可在平台成功接受命中关闭后将当前贴图设为鼠标穿透，tray 的同一条目先恢复命中后再显示、聚焦和重绘；穿透状态只存在于当前窗口生命周期；多贴图 |
| 导出 | Overlay 的复制/保存可在会话内选择原图或标注合成图（默认合成）；贴图复制/保存只输出当前 `PinWin.image` 像素，不烧录窗口缩放、透明度、OCR 词框或客户区 UI。文件支持 PNG（默认）、JPEG（可配置质量）和无损 WebP；内存与系统剪贴板固定 PNG（wl-copy/xclip） |
| 全局热键 | `GlobalHotkeyHub` 在 GUI 事件循环线程持有 `global-hotkey` manager；设置窗口可录制区域和全屏主键，保存时先预注册新组合、再撤销旧组合，默认 F2/F3；Ctrl+N/Ctrl+Shift+S 保持区域备用键。tray 菜单在创建及设置成功时显示已保存主键，但实际注册仍以能力摘要/诊断为准。Windows/macOS 为原生后端、Linux 仅 X11；纯 Wayland 仍使用 tray 或 `pinora capture` IPC 降级 |
| 单实例 | Unix 使用 `flock` + Unix socket；非 Unix 使用文件锁 + 本地回环 TCP 端口文件，均支持 Activate/CAPTURE/QUIT；真实 Windows/macOS 进程行为仍待探针 |
| 帧缓存 | 空闲预截；热键命中以所有权移交预处理帧，避免复制全屏图像与双 XRGB 缓冲；暂停以代际拒绝晚到帧 |
| 基础标注 | Overlay 选区内：选择、矩形/圆角矩形/直线/箭头/画笔/椭圆/序号/马赛克/区域模糊/文本/截图内取色；文本 `Shift+Enter` 换行、`Enter` 提交、`Esc` 取消，非空草稿在重选/切换工具前提交；`V` 选择，选中对象可拖动或以方向键移动（Shift 为 10 像素），`Q` 圆角矩形，`L` 直线，`N` 序号，`B` 区域模糊，`F` 切换后续封闭图形的半透明填充，C 颜色，I 取色，+/- 线宽，Delete/Backspace 删除选中项；`Ctrl+Z` 撤销，`Ctrl+Shift+Z`/`Ctrl+Y` 重做，工具栏“清空”可一次撤销/重做整个标注文档 |
| 系统托盘 | 截图、1/3/5 秒延时区域截图及取消、显示器指定全屏、可用 xcap 后端的最多 20 个经清洗窗口截图候选、设置、历史、诊断、动态贴图列表、显示/隐藏/关闭全部贴图、退出；贴图列表按进程内最近使用排序，只显示通用序号、可见性和鼠标穿透状态，选择穿透贴图时复用其既有窗口并先恢复鼠标交互；菜单禁用状态项与图标 tooltip 显示最近一次受控截图/OCR/导出/贴图鼠标状态，另有本次启动的截图/热键/图像剪贴板/OCR 能力摘要；诊断面板只显示受控能力、固定状态、稳定错误码和恢复建议（tray-icon；真实跨平台菜单与窗口枚举仍待探针） |
| 辅助面板主题 | 设置、历史和诊断三个自绘 `Panel` 使用共享 `PanelTheme` token；`Light`/`Dark` 强制覆盖系统外观，`System` 读取窗口初始主题并仅对 `ThemeChanged` 的明确事件刷新，未知系统外观稳定回退 Dark。设置草稿即时预览；历史/诊断仅在原子保存成功后同步。Overlay、贴图、原生菜单和标题栏不在此主题范围内 |
| 后台驻留与窗口隔离 | 启动后只保留托盘、可用全局热键、IPC 与帧缓存，不自动截图；无法创建托盘时以 `CapabilityUnavailable` 退出；所有辅助窗口必须由 `window_policy` 工厂创建并请求跳过任务栏/Dock，真实桌面验证仍待完成 |
| 贴图控制 | L 锁定，`[` `]` 透明度（压暗近似）；`O` 本地 OCR；`T` 词框 |
| OCR | 系统 `tesseract` CLI；可持久化选择 Auto、English、SimplifiedChinese 与 0..=100 的低置信词框阈值（默认 60）；同资产版本和语言的 accepted 结果进程内复用；全文复制剪贴板；词框叠加且低置信仅改变未选中词的告警描边；缺引擎或指定本机模型时受控降级 |

## 2026-08-01 接管审计事实

- 接管初期 Unix 单实例路径直接依赖 Unix socket；后续实现已将 `OsSingleInstance` 与 `forward_ipc_frame` 按 Unix/非 Unix 条件编译，根 `src/main.rs` 只依赖其抽象入口。真实 Windows/macOS 的并发启动、权限和异常恢复仍待桌面探针。
- `crates/pinora-app/src/desktop_shell.rs` 当前约 3679 行，仍集中承载 winit/softbuffer 窗口事件、截图编排、Overlay 绘制、标注输入、贴图生命周期、OCR 触发、托盘和 IPC 轮询；045/046 已将历史和设置窗口的资源、草稿/预览缓存、resize、存储调用和呈现迁至专属适配器，但 Overlay/贴图仍在 shell 中，单体化风险保持开放。
- 当前依赖树把 `gtk`/`tray-icon`、`xcap`/PipeWire、`winit`/`softbuffer` 和 Linux CLI 后端直接放入 `pinora-app`；没有 Windows/macOS/Linux 适配器边界。
- `cargo fmt --check`、`cargo check --workspace` 和 `cargo clippy --workspace --all-targets -- -D warnings` 已于 2026-08-02 通过；当前 `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace` 通过 app 240 个、core 88 个单元测试，另有 2 个真实桌面测试被忽略；仍没有 GUI 端到端测试。
- 接管早期 Windows target 曾被 GTK 的 `gdk-pixbuf-sys`/`glib-sys` pkg-config 阻塞；GTK 已改为 Linux target 依赖，2026-08-02 的 `cargo check --workspace --target x86_64-pc-windows-msvc` 已通过。该事实只证明交叉编译，不证明 GUI 能力。
- OCR 通过 `tesseract` 子进程和临时 PNG 工作；适配器已持有自身 `Child`，支持协作式取消、30 秒截止时间、16 MiB 输出上限和 RAII 临时文件清理，不再调用外部 `kill`。贴图与 Overlay UI 已经通过 `OcrJobService` 提交到 `JobSupervisor`，结果交付受 owner、终态和 `AssetRef` generation 门禁保护；worker 不触碰窗口或剪贴板。
- 截图后端自动选择 KDE `spectacle` → xcap → `Unavailable`；两者不可用时保留后端失败摘要并由 provider 返回 `CapabilityUnavailable`，`fake` 只能通过显式测试/开发注入使用。
- `docs/Pinora-开发设计文档.md` 已于 2026-08-01 更新为 v1.0 生产重构基线：明确当前实验实现、目标端口/适配器架构和待验证技术决策；文档不代表任何新功能已经交付。
- 2026-08-02 的 049/050 本地实现将截图方式、Overlay 初始选区和窗口呈现方式分离：历史条目在完整性校验后通过普通编辑窗口进入全图标注，不重新捕获屏幕；失败保留历史窗口并恢复帧缓存。
- 2026-08-02 的 050/054 实现移除了空闲控制窗口与启动自动截图，并将全部生产窗口构造收敛至 `window_policy`：Windows 请求跳过任务栏、X11 请求 Utility、macOS 使用 Accessory/`LSUIElement`，KDE Wayland 在映射后额外请求 `skipTaskbar`/`skipPager`；隐藏 display-handle 也受创建前策略约束。提交 `609b862` 的三平台 CI 已验证该代码可通过各原生 runner 的静态门禁。这不等于真实任务栏、Dock 或合成器验收。
- 2026-08-02 的 057 实现为 `CaptureProvider` 增加了经快照验证的窗口枚举/捕获契约：xcap 在点击后按内部窗口 ID、几何、显示器和缩放重新枚举验证，最小化、消失或拓扑变化不会回退为显示器截图；tray 仅在启动时保存最多 20 个经过清洗的候选。窗口捕获和 Overlay 创建失败会回到 tray 空闲态，后台线程只回传稳定错误码，避免后端错误文本写入日志。提交 `b60ebf0` 的本地全量门禁与 GitHub CI `30736791038`（Linux/macOS/Windows）均通过；真实窗口像素、权限、任务栏/Dock 与合成器行为尚未验证。
- 2026-08-02 的 058 实现取消了 `LoadingState` 向 `about_to_wait` 返回可恢复错误的路径：区域、全屏、指定显示器、窗口和延时截图的 worker 错误、断开、预览不一致和 Overlay 建立失败均释放当前会话并回到 tray；延时会话保留贴图恢复优先级，日志只输出稳定错误码。提交 `36ee681` 的全量离线门禁与 GitHub CI `30737231248`（Linux/macOS/Windows）均通过；真实桌面窗口销毁、tray 连续驻留和任务栏/Dock 行为仍未验证。
- 2026-08-02 的 059 实现新增全虚拟桌面截图契约：`AllDisplays` 使用开始时显示器物理 bounds 的安全外接矩形和 `pinora:virtual-desktop` 来源，KDE 只接受尺寸严格匹配的单次 `spectacle -f` PNG，xcap 明确拒绝而不拼接多时刻帧。多显示器 tray 提供“所有显示器截图”；该路径不消费单屏帧缓存，成功时以经过 `window_policy` 隔离的无边框虚拟桌面 Overlay 呈现。提交 `085e615` 的本地严格门禁与 GitHub CI `30737751448`（Linux/macOS/Windows）通过；真实双屏 KDE 像素、空洞区域、Overlay 映射、tray、任务栏/Dock、HiDPI 和合成器行为仍未验证。
- 2026-08-02 的 060 实现将贴图右键菜单限制在现有贴图客户区内，未使用原生平台菜单或额外 `Window`。编辑复用经 `window_policy` 创建的 Overlay，提交时保留 `PinId`、替换图片引用、推进 `AssetRef` generation、关闭旧 OCR/导出 owner 并刷新渲染缓存；取消、裁剪失败和重新截图恢复被隐藏的原贴图。离线测试覆盖事务、菜单和编辑路径；真实任务栏/Dock、KWin、HiDPI、置顶和输入延迟仍未验证。
- 2026-08-02 的 061 删除了未受托盘监督的独立贴图会话、区域 Overlay 和区域工作流公开 API。`pinora-app` 的唯一公开 GUI 会话入口为 `run_desktop_shell`，它在构造 `DesktopApp` 前要求成功创建系统 tray；贴图尺寸计算迁移为纯 `pin_layout`。`window_policy` 单元测试扫描生产源，确保只有该模块构造 `EventLoop` 或直接调用 `create_window`。这些事实只证明代码可达路径与静态边界，不证明真实窗口管理器最终不会显示任务栏/Dock 项。
- 2026-08-02 的 062 为 Overlay 新增截图内取色：`I` 或工具栏滴管进入取色器，点击当前选区将原始 RGBA 像素设为后续画笔颜色、恢复原工具并通过当前会话的受监督剪贴板任务复制 `#RRGGBB`。取色不烧录预览、不重捕获、不新建窗口，也不推进标注文档 revision。工具栏依据画布宽度换行，空间不足时不绘制裁切控件；真实剪贴板、HiDPI 和输入延迟尚未验证。
- 2026-08-02 的 063 为 Overlay 增加 `L` 直线和 `N` 序号：直线仅在有效拖拽后提交；序号一次点击立即提交，使用当前颜色/线宽派生的圆形标记与对比色数字，起始值可在 `1..=99_999` 设置，达到上限后停止新建以避免重复值。两工具均复用现有标注文档、草稿预览与栅格化路径；序号双击不会复制图片，且没有新窗口、事件循环、截图或后台任务路径。真实工具栏、HiDPI、输入帧时间、tray 与任务栏/Dock 行为尚未验证。
- 2026-08-02 的 064 为 Overlay 增加 `Q` 圆角矩形：一次有效拖拽提交，提交时把当前线宽派生且受短边约束的半径保存到对象；距离场描边对已存半径再次钳制，草稿预览与最终合成保持一致。该工具没有新窗口、事件循环、截图或后台任务路径，生产建窗仍只经 `window_policy`；真实圆角视觉、HiDPI、输入帧时间、tray 与任务栏/Dock 行为尚未验证。
- 2026-08-02 的 065 为 Overlay 矩形、圆角矩形和椭圆增加可切换的半透明填充：`F` 或现有工具栏按钮只更新当前会话样式和工具栏选中态，不推进标注 revision、不提交任务且不创建窗口；有效提交将当前 RGB 和固定 alpha 96 冻结到对象，预览与最终烧录先填充、后描边并统一经 alpha-over 合成。离线像素、工具栏和 `window_policy` 守卫已通过；真实任务栏/Dock、tray、HiDPI 和大面积连续拖拽帧时间仍未验证。
- 2026-08-02 的 066 将所有辅助窗口映射收敛到 `window_policy`：工厂无条件隐藏创建，唯一展示入口先调用 `set_visible(true)` 再执行 KWin 映射后 `skipTaskbar`/`skipPager` 请求；截图 Overlay、贴图首次/批量/编辑恢复、历史和设置均已迁移，隐藏 display handle 不可展示。递归源码守卫已拒绝策略模块外的建窗、事件循环和显式可见调用；真实 Windows/macOS/X11/KDE Wayland 任务栏/Dock、tray、首帧和焦点仍未验证。
- 2026-08-02 的 067 为 Overlay 增加 `B` 与工具栏入口的区域模糊：有效拖拽提交随线宽派生且受 `4..=24` 约束的冻结半径，预览和最终烧录均从原始不可变截图使用分离滑动盒模糊采样，且只写入选择矩形。离线回归覆盖退化拖拽、半径冻结、预览逐字节一致、反向/边界坐标、区域外字节不变和工具栏/窗口策略守卫；真实 4K/HiDPI 连续拖拽帧时间、任务栏/Dock、tray、焦点与合成器行为仍未验证。
- 2026-08-02 的 068 将 Overlay 标注预览拆为“当前源选区的已提交 RGBA 层”与“当前草稿叠加”：缓存键为源选区和 `AnnotationRevision`，草稿移动不再重新烧录全部历史标注；提交、撤销、重做、重选或无效裁剪会重建或清除缓存。核心草稿叠加与完整预览逐字节等价，马赛克和 Blur 始终从原始裁剪采样；没有新增窗口、事件循环、截图或 worker。真实 4K/HiDPI 帧时间、峰值内存、tray、任务栏/Dock、焦点与合成器行为仍未验证。
- 2026-08-02 的 069 将 `AnnotationDoc` 内部历史扩展为新增与整体清空事务：非空清空保存原绘制顺序并推进 revision，单次撤销完整恢复、单次重做再次清空；空清空不改变 revision 或 redo 分支，后续新标注按既有契约清除 redo。Overlay 工具栏“清空”先取消草稿、再仅重绘现有 Overlay；没有新窗口、系统菜单、截图或 worker。应用空闲必须仅以 tray 常驻，Overlay、贴图和所有辅助层禁止成为任务栏、Dock 或分页器项；静态策略守卫和离线测试已通过，真实原生桌面仍待验收。
- 2026-08-02 的 070 增加 `Select` 标注工具：点击按反向绘制顺序命中最上层对象，选择框只在既有 Overlay 呈现帧绘制；Delete/Backspace 通过保存原索引和对象快照的内存事务删除，undo 原位恢复、redo 再次删除。提交、撤销、重做、清空、重选、切换工具和 Overlay 生命周期均清除瞬态选择；没有新窗口、系统菜单、截图或 worker，tray-only 与 `window_policy` 边界不变。离线命中、事务、工具栏和窗口策略已通过；真实点击、高 DPI、帧时间及任务栏/Dock/分页器仍未验证。
- 2026-08-02 的 071 在既有选择工具中增加对象移动：`AnnotationDoc` 的替换事务以原索引、旧对象和新对象实现单次可撤销/重做编辑；所有对象仅平移几何并保留视觉字段。Overlay 拖动预览独立于文档、revision、导出、贴图和 OCR 输入，释放后才提交；预览缓存从不可变原始裁剪合成，马赛克/Blur 不读取已提交标注像素。方向键优先移动选中对象，Shift 步长为 10，未选中时保持选区移动。该功能没有新增窗口、事件循环、系统菜单、截图或 worker，仍只复用 `window_policy` 的隐藏创建与展示边界；真实高 DPI、帧时间、tray 与任务栏/Dock/分页器仍未验证。
- 2026-08-02 的 072 为区域 Overlay 已确认且未标注的选区增加四边/四角调整：八方向几何、最小尺寸和 bounds 约束集中于 `SelectionSession`；热区在现有呈现帧绘制，最近中心命中并复用 2 像素/32ms 拖动节流。拖动期间不更新导出、OCR、贴图或后台任务输入，抬起/快捷完成时才同步当前源选区；有草稿或已提交标注时保持既有重选路径。没有新增窗口、事件循环、系统菜单、截图或 worker，tray-only 与 `window_policy` 边界不变；真实热区、HiDPI、帧时间与任务栏/Dock/分页器仍未验证。
- 2026-08-02 的 073 为当前 Overlay 增加选区物理像素读数：`W… H… X… Y…` 中的尺寸来自源图，坐标为 `buf_rect_to_src` 后叠加当前捕获会话 `display_origin`，支持负全局 origin。独立读数模块优先将 panel 放在选区上方并避让下方工具栏；极小画布仍限制在既有帧内。读数的新旧 bounds 均参与脏区恢复，普通拖选、键盘移动和八方向调整复用原有节流；没有新增窗口、事件循环、系统菜单、截图或 worker，tray-only 与 `window_policy` 边界不变；真实可读性、HiDPI、帧时间与任务栏/Dock/分页器仍未验证。
- 2026-08-02 的 074 为 Overlay 文本标注增加多行与明确提交边界：`Shift+Enter` 在草稿中插入换行、`Enter`/`Ctrl+Enter` 提交、`Esc` 显式取消。文本绘制、fallback、bounds 和命中均共享行距，空白行保留垂直空间；外部重选或工具切换前先提交非空草稿，空白草稿可安全清除。没有新增窗口、事件循环、系统菜单、截图或 worker，tray-only 与 `window_policy` 边界不变；真实输入法、字体、HiDPI、帧时间与任务栏/Dock/分页器仍未验证。
- 2026-08-02 的 075 增加 tray 最近关闭贴图撤销：内存快照不包含窗口或任务句柄，关闭先使旧 owner/runtime Pin 失效；恢复通过新的 `PinId`、asset 和既有 `spawn_pin` 重新创建受策略保护的贴图窗口，创建失败保留快照重试。没有新增窗口类型、事件循环、截图或 worker；真实 tray、首帧、HiDPI、焦点与任务栏/Dock/分页器仍未验证。
- 2026-08-02 的 076 为既有贴图增加四边/四角等比缩放与 100% 原图恢复：八方向几何、比例、边界和手动回退锚点在 `pin_layout` 纯逻辑中覆盖；当前 Pin 先用 `drag_resize_window`，平台不支持时才在同一窗口内请求尺寸。原生 resize 不再重复改左/上位置，客户区指针按热区变化且去重，OCR Ctrl+拖选优先于尺寸操作。双击或客户区菜单 `100%` 仅修改当前未锁定贴图；实际尺寸/缩放变化才失效缓存并同步领域 transform。没有新增窗口、事件循环、展示入口、截图或 worker，tray-only 与 `window_policy` 边界不变；真实原生 resize、HiDPI、焦点、任务栏/Dock/分页器和帧时间仍未验证。
- 2026-08-02 的 077 为普通 Overlay 新贴图增加来源避让：`pin_layout` 仅以当前完整捕获的物理像素范围，在右、左、下、上顺序寻找完整容纳且不与来源选区相交的位置；无候选时将来源左上角确定性钳制于该范围，且使用宽位中间值避免坐标溢出。策略仅在普通 `OverlayFinish::Pin` 创建既有贴图窗口前执行，复制、保存、贴图编辑、历史重新贴图、关闭撤销和手动位置操作均不变。没有新增窗口、事件循环、展示入口、截图或 worker，tray-only 与 `window_policy` 边界不变；真实初始位置、跨屏映射、HiDPI、焦点、任务栏/Dock/分页器和帧时间仍未验证。
- 2026-08-02 的 078 为现有 tray 增加最近异步状态反馈：受限纯模型以静态中文文案和稳定 `ErrorCode` 映射生成截图、延时截图、OCR 与导出的进行中、成功、失败/取消状态；禁用菜单项与 tooltip 共享同一文本，不包含图像、OCR 文本、路径或原始错误。桌面壳只在启动或 owner/`AssetRef` 匹配的完成分支更新状态；OCR、导出 worker 错误也复核当前资产，陈旧或关闭 owner 的失败被丢弃。没有新增窗口、事件循环、worker、通知或网络路径，tray-only 与 `window_policy` 边界不变；真实 tray 动态刷新、任务栏/Dock/分页器和原生桌面体验仍未验证。
- 2026-08-02 的 079 为已有 tray 增加本次启动的能力摘要：截图和系统图像剪贴板复用 bootstrap `CapabilitySnapshot`，全局热键以 `GlobalHotkeyHub` 的实际注册结果覆盖平台猜测，本地 OCR 仅执行无进程的 PATH 检查。禁用菜单项由固定中文标签生成，不读取 runtime notes、路径、后端错误、OCR 文本或剪贴板内容；没有新窗口、事件循环、worker、外部进程、通知或网络路径，tray-only 与 `window_policy` 边界不变。真实 tray 可见性、读屏、权限和窗口管理器行为仍未验证。
- 2026-08-02 的 080 为桌面异步 PNG 保存增加 UTC 可读命名：分配器使用固定 `Pinora_YYYYMMDD_HHMMSS[_NNN].png` 格式，跳过同秒与已存在的候选；Overlay、贴图编辑、贴图自动保存和贴图菜单保存均在提交既有导出 worker 前使用它。没有改变编码、原子写入、历史、任务身份、窗口或 tray 生命周期；普通跨进程文件系统竞态仍未消除，真实桌面验证也未完成。
- 2026-08-02 的 081 将 `GlobalHotkeyHub` 改为在 `winit` GUI 事件循环线程持有并释放 `GlobalHotKeyManager`，不再用仅 Linux 的应用侧线程转发事件。F2/Ctrl+N 是核心注册，Ctrl+Shift+S/F3 保持可选且状态说明来自实际注册结果；Windows/macOS 不再因应用编译期开关被直接禁用，Linux 仍只声明 X11，纯 Wayland 使用 tray 或 IPC 降级。Pinora 没有新增 `winit` 窗口；Windows 依赖后端会使用隐藏 `WS_EX_TOOLWINDOW` 消息窗口，源码请求跳过任务栏但尚缺实机证据。离线事件过滤、不可用降级、window policy、严格 workspace 门禁和 Windows target 编译已通过；真实 Windows/macOS/X11 热键、Wayland Portal、冲突、权限与睡眠恢复尚未验证。
- 2026-08-02 的 082 将设置 schema 升级为 v2：18 字节 v1 记录保留既有字段并以 `Auto` 迁移，后续原子保存写入 19 字节 v2。设置窗口可选择 Auto、English、SimplifiedChinese；桌面壳在提交 OCR worker 前冻结 runtime 中的预设。自动模式仅组合本机 `chi_sim`/`eng`，指定模式缺模型时返回 `CapabilityUnavailable`，不下载模型、不回退；OCR 失败日志只写稳定错误码。离线 codec、面板、模型选择、worker 冻结和 workspace 门禁已通过，真实模型安装、设置输入、剪贴板与桌面体验仍未验证。
- 2026-08-02 的 084 将设置 schema 升级为 v3：v1/v2 文件保留既有字段并新增默认区域 F2、全屏 F3 主键；v3 以受限物理键和修饰键编码两个主键。设置窗口使用同一辅助窗口进入录制状态，录制优先于窗口内截图快捷键，拒绝裸字母、重复或占用 Ctrl+N/Ctrl+Shift+S 的组合。热键 hub 在新键全部预注册后才释放旧键，设置写入失败时尝试恢复旧键；不可用后端仍可保存配置但不报告已注册。没有新增窗口、事件循环、线程、网络或权限绕过；真实平台注册、冲突与任务栏/Dock 行为仍未验证。
- 2026-08-02 的 085 增加 tray“诊断”入口和短生命周期的本地 `Panel`：诊断内容只由平台常量、公开能力布尔值、`GlobalHotkeyHub` 实际注册结果、`tesseract_available` 和 `TrayFeedback` 固定枚举生成；`CapabilitySnapshot.notes`、原始错误、路径、截图像素、OCR 文本、剪贴板内容和窗口/显示器标识均不会进入模型或渲染。失败只显示稳定 `ErrorCode` 与固定恢复建议，成功/进行中不伪造错误；打开期间受控 tray 反馈会刷新面板。窗口仍由 `window_policy` 先隐藏创建并在关闭/Esc/截图时释放，未新增事件循环、线程、网络或持久化。离线测试与静态策略门禁通过；真实 tray、焦点、读屏、权限状态及任务栏/Dock/分页器隔离仍未验证。
- 2026-08-02 的 087 为设置、历史和诊断 `Panel` 建立共享 `PanelTheme`/`PanelThemeState`：`Light`/`Dark` 不受系统外观影响，`System` 使用窗口创建时的 `Window::theme()` 与后续 `ThemeChanged`，未知状态稳定回退 Dark。设置面板从草稿即时解析调色板；历史/诊断只在原子设置保存成功后收到新偏好，失败不会传播。三个窗口仍使用已有 `window_policy` 创建和展示，没有新增窗口、事件循环、线程、网络、持久化形状或设置 schema。浅深 XRGB 帧、草稿预览、保存成功/失败、主题事件解析、窗口策略、workspace 严格门禁和 Windows target 编译已验证；真实系统主题事件、原生标题栏、tray、读屏、色彩管理、HiDPI 与任务栏/Dock/分页器行为仍需桌面会话验证。
- 2026-08-02 的 088 将设置 schema 升级为 v4：24 字节记录末字节严格保存默认贴图置顶 `0`/`1`，v1/v2/v3 读取均保留旧字段并默认迁移为置顶，损坏 v4 字节保留源文件并使启动回退内存默认。设置面板新增明确 `OFF`/`ON` 控件、左右键编辑和扩展后的无重叠布局；桌面壳只在原子保存并回读成功后更新后续新贴图的领域与窗口层级，已打开贴图和关闭恢复贴图分别保持自身状态。没有新增窗口、事件循环、线程、网络、依赖或平台权限路径。codec、面板、层级选择、窗口策略、严格 workspace 门禁和 Windows target 编译已验证；真实置顶请求、任务栏/Dock/分页器、焦点和 HiDPI 仍需原生桌面会话验证。
- 2026-08-02 的 089 增加 tray 动态“贴图列表”子菜单：条目按内部 `PinId` 稳定排序，但菜单只显示“贴图 N”和可见/隐藏状态，不保留或显示标题、图像、OCR、路径、坐标或内部 ID。空列表为禁用占位；用户选择条目时只在当前 GUI 线程找到既有 `PinWin`，再经 `window_policy::show_auxiliary_window` 显示、聚焦和重绘。创建、关闭、批量显示/隐藏、延时截图隐藏/恢复、编辑隐藏/恢复及编辑替换显示后均刷新列表；批量关闭只刷新一次。没有新增窗口、事件循环、线程、截图、持久化、依赖或展示入口。离线 tray 映射/标签/排序、桌面壳和窗口策略门禁已验证；真实原生 tray 更新、单贴图焦点、任务栏/Dock/分页器和 HiDPI 仍需桌面会话验收。
- 2026-08-03 的 090 将 tray 贴图列表排序改为最近使用优先：`PinWin` 与桌面壳只保留进程内饱和 recency 计数，新建、`Focused(true)` 和 tray 唤起更新该值；tray 以 recency 降序、PinId 升序处理并列。该值不进入领域、设置、历史、菜单标签或日志；焦点更新仅重建既有子菜单，既有 `window_policy` 展示入口不变。离线排序、饱和计数、桌面壳和窗口策略门禁已验证；真实原生焦点事件、tray 刷新、任务栏/Dock/分页器和 HiDPI 仍需桌面会话验收。
- 2026-08-03 的 091 将设置 schema 升级为 v5：25 字节记录在 v4 字段后追加 0..=100 的 OCR 置信度阈值，默认 60；v1-v4 读取均保留原字段并以默认阈值迁移，v5 数值越界逐字段修复，保存仍采用临时文件、同步、原子替换和回读。`ocr_presentation` 仅从既有 `OcrWord.confidence` 派生普通、低置信、选中三种词框状态；未知/非有限/越界置信度不伪装为低置信，选中状态优先，OCR 原始词、全文和框选复制、缓存、任务不改变。桌面壳仅在保存成功后更新 runtime 并请求已有贴图重绘，不重跑 OCR、不创建窗口或 worker。core/codec/面板/呈现测试、严格 workspace 门禁和 Windows target 编译已验证；真实 Tesseract 模型的阈值可用性、HiDPI 视觉、连续重绘帧时间与任务栏/Dock/分页器仍需原生桌面会话验收。
- 2026-08-03 的 092 将设置 schema 升级为 v6：27 字节记录在 v5 字段后追加导出格式（PNG/JPEG/无损 WebP）和 1..=100 的 JPEG 质量，默认 PNG/90；v1-v5 保留既有字段并以默认格式/质量迁移，未知格式保留源文件并拒绝读取，非法质量逐字段修复。桌面壳在提交既有 `ExportJobService` 前冻结路径、格式和质量，文件名由格式唯一生成扩展名；PNG/WebP 保留 RGBA，JPEG 在 worker 内以确定性白底合成为 RGB。所有格式沿用同目录临时文件、同步、原子替换与结果门禁；系统剪贴板仍只编码 PNG。只有受管 PNG 成功结果写入 PNG-only 历史，JPEG/WebP 不伪造索引记录；tray 反馈泛化为文件保存。没有新增窗口、事件循环、外部进程、权限或网络路径。离线编码/codec/面板/命名/任务/历史/反馈测试、workspace 严格门禁与 Windows target 编译均已通过；真实文件查看器兼容性、色彩、性能、HiDPI 与任务栏/Dock/分页器仍需原生桌面会话验收。
- 2026-08-03 的 093 在既有 Overlay 自绘工具栏增加会话内 `RAW`/`ANN` 导出来源切换，默认 `ANN` 保持原 Copy/Save 的标注合成输出；Copy/Save 在关闭 Overlay、提交既有 worker 前冻结所选 RGBA 帧。原图是当前选区未标注像素，合成图在提交草稿后烧录标注文档。贴图和 OCR 始终固定合成图，贴图 Copy/Save 只克隆当前 `PinWin.image`，不截取窗口表面，因此不混入缩放、透明度、OCR 词框或客户区菜单。来源状态不持久化、不改变格式/质量、历史、资产/所有权门禁、窗口策略或建窗路径。离线像素、工具栏、桌面壳与 workspace 严格门禁及 Windows target 编译已通过；真实高 DPI 可发现性、帧时间、系统剪贴板/文件结果和任务栏/Dock/分页器仍需原生会话验收。
- 2026-08-03 的 094 为既有异步图片文件保存增加 tray 取消：菜单仅在至少一个 `JobState::Running` 的 `PendingExportAction::SaveImage` 存在时启用，一次只向这些 job 发送协作式取消；CopyImage、CopyText、OCR、截图和其他 owner 保持原生命周期。保存 worker 在编码前、编码后和原子 `rename` 前检查 token；取消发生在发布前时 `AtomicExportTemp::Drop` 清理自身临时文件，已完成发布的目标绝不由迟到取消删除。取消请求后 pending 映射保留至 worker terminal 结果，反馈区分“正在取消文件保存”与“文件保存已取消”，且不携带路径或用户内容。没有新增窗口、事件循环、外部进程、设置或持久化字段；原生慢盘取消延迟、tray 刷新和发布竞态仍待桌面验证。
- 2026-08-03 的 095 在 `PinWin` 增加只存于当前窗口生命周期的 `PinMouseMode`，客户区 `PASS` 仅在 `Window::set_cursor_hittest(false)` 成功后提交穿透状态，并清除菜单、拖动、缩放、OCR 选择和双击瞬态状态。动态 tray 列表用无内容标签表示穿透；选择该条目时先请求 `set_cursor_hittest(true)`，失败则保持状态、可见性与焦点不变并显示固定能力受限反馈，成功后才复用既有 `window_policy` 展示、聚焦和重绘路径。该状态不进入领域、设置、历史、关闭恢复快照或日志，没有新增窗口、事件循环、线程、进程、依赖或展示入口。离线菜单、标签、状态提交、反馈、workspace 严格门禁与 Windows target 编译已验证；真实鼠标输入区域、tray 刷新、焦点、流畅性及任务栏/Dock/分页器仍需原生桌面会话验收。
- 2026-08-03 的 096 将设置 schema 升级为 v7：29 字节记录在 v6 字段后追加 1..=3650 天的历史保留期，默认 30 天；v1-v6 读取均保留既有字段并以默认保留期迁移，v7 非法天数逐字段修复。`HistoryIndex::expire_before` 只将创建时间早于应用层 Unix 时间截止点的活动条目标记为 tombstone；桌面壳在启动、设置原子保存成功和受管 PNG 历史写入后统一协调数量、容量与保留期，先原子保存新 tombstone，再复用受管直属 PNG 白名单清理。系统时间不可读取时跳过时间淘汰；索引写入、文件删除或最终压缩失败保留可重试状态，不触碰外部/嵌套/活动同名文件。core/codec/面板/历史事务和 workspace 门禁已验证；真实系统时钟跳变、断电恢复、跨平台文件系统、历史窗口刷新与桌面性能仍待原生探针。
- 2026-08-03 的 097 将设置 schema 升级为 v8：37 字节记录在 v7 字段后追加 16 MiB..=64 GiB 的 `history_max_bytes`，默认 1 GiB；v1-v7 读取保留既有字段并以默认容量迁移，v8 非法容量只修复该字段。设置面板以 MiB/GiB 显示、以 64 MiB 步进；桌面壳在启动与成功保存设置时把容量传入 `HistoryStore`，受管 PNG 新增后继续复用同一数量/容量/保留期 tombstone 协调器。容量下调时先原子保存最旧优先的 tombstone，再只清理直属受管 PNG；保存、删除或最终压缩失败均保留可重试状态，不触碰外部、嵌套或活动同名文件。core/codec/面板/历史事务与 workspace 门禁已验证；真实断电、只读/网络文件系统、历史窗口刷新、跨平台桌面与性能仍待原生探针。
- 2026-08-02 的 086 使 tray 区域/全屏截图菜单使用当前已保存的受限 `HotkeyBinding`，初始化及设置原子写入成功后都会原地更新两个既有 `MenuItem`。热键重绑失败或设置保存失败发生在更新前，因此保留旧菜单文字与旧运行时映射；菜单文字不表示 OS 注册成功，能力摘要和诊断仍读取 `GlobalHotkeyHub` 实际状态。未新增 tray、窗口、事件循环、线程、进程、网络或依赖；真实原生 tray 文本刷新与桌面行为仍未验证。
- 2026-08-02 的 083 为 `OcrJobService` 增加进程内结果复用：只有通过 owner、终态和 `AssetRef` generation 门禁的成功 OCR 才以完整 asset 与冻结语言进入缓存。缓存为最多 8 条、2 MiB 估算总量、512 KiB 单条上限的内存 LRU 风格队列；命中由 service 保证不创建新 worker，桌面壳仍走原有词框、全文复制与 tray 成功交付。缓存不持久化、不感知外部模型文件变化；离线缓存、服务和 workspace 门禁已通过，真实连续操作、内存峰值和桌面体验仍未验证。
- GitHub Actions CI `30732620836`、`30732765136`、`30732906042`、`30733684203`、`30734154282`、`30734583848`、`30734867309` 与 `30735354166` 已于 2026-08-02 在 Linux、macOS、Windows 原生 runner 通过格式、workspace 编译、严格 Clippy 和单元测试；这些运行未创建 GUI 会话，不能作为任务栏、Dock、窗口交互、KWin 行为、真实多显示器或渲染延迟的证据。
- `pinora-core::asset` 已于 2026-08-01 新增 `AssetGeneration` 和 `AssetRef` 领域契约；它只组合既有 `ImageId`，可判定陈旧结果，已用于桌面贴图及 Overlay OCR、复制、保存任务的结果门禁。
- `pinora-core::job` 与 `pinora-app::JobSupervisor` 已于 2026-08-01 新增：任务元数据绑定 `JobId`、关联 ID、`AssetRef`、领域 owner、类型和截止时间；监督器可协作式取消、关闭 owner、标记超时并拒绝终态或陈旧版本结果。桌面 OCR、导出和剪贴板均已接入，但这不代表所有后台进程均已在真实桌面环境验证。
- `pinora-app::OcrJobService` 已于 2026-08-01 接入 `desktop_shell`：可注入 runner 在 worker 中执行 OCR，主线程轮询通过 `JobSupervisor` 后才交付结果，覆盖失败、owner 关闭、超时和 generation 失效。贴图关闭、Overlay 取消/再截和应用退出均会取消对应任务；服务契约测试仍不等价于真实窗口 E2E。
- `pinora-app::image_sink` 已于 2026-08-01 收敛系统剪贴板子进程：输入和 stderr 使用 RAII 临时文件，适配器直接轮询 `Child`，超时只对拥有的 child 执行 `kill`/`wait`；其图像/文本复制入口已由桌面 `ExportJobService` 调用。
- `pinora-app::ExportJobService` 已于 2026-08-01 接入 `desktop_shell`：统一监督 PNG 保存、图像剪贴板和 OCR 文本剪贴板输入，主循环按 owner、job ID、资产 generation、截止时间和终态门禁结果；服务契约与纯逻辑测试仍不等价于真实窗口 E2E。
- `pinora-app::save_png_file` 已于 2026-08-01 使用同目录临时文件、文件 `sync_all`、rename 发布和目标可读性校验；未提交临时文件由 RAII 删除。该事实只在 Linux 本地文件系统测试，未证明跨平台覆盖或断电后目录持久性。
- `OcrJobService` 与 `ExportJobService` 已于 2026-08-01 保存自己创建的 worker 句柄，正常轮询会回收结束线程；桌面退出先取消、最多等待 2 秒并输出取消/join/panic/残留计数。协作式 worker 若不响应取消会被如实报告为残留，不能视为已回收。
- `pinora-core::annotate` 已于 2026-08-01 新增 `AnnotationRevision`：新文档从非零版本开始，有效提交、非空撤销和非空重做均单调推进且在 `u64::MAX` 饱和；标注集合与 redo 栈只暴露只读查询。Overlay 已为确认选区建立稳定派生 `ImageId`，将 revision 映射为 `AssetRef.generation` 并用于 OCR、复制和保存；有效编辑、撤销、重做或重选会拒绝晚到结果。贴图尚无标注回编辑。
- `pinora-core::settings` 与 `pinora-app::SettingsStore` 已于 2026-08-02 建立版本化设置与原子文件基础：格式有 magic、schema 与长度校验，非法数值逐字段修复，损坏/未知版本保留源文件并回退内存默认。035 已将 `pin_limit` 和新贴图默认不透明度接入 runtime/desktop shell；041 新增独立自绘设置窗口，支持主题、历史上限、贴图上限和默认不透明度的键盘/鼠标编辑、取消、原子保存和失败回滚，并在保存成功后应用运行时策略；082 将 schema v1/v2 迁移和 OCR 语言预设纳入同一事务。系统主题跟随、原生控件无障碍和跨平台目录策略仍未验证。
- `pinora-core::history` 与 `pinora-app::HistoryStore` 已于 2026-08-02 建立历史索引基础，并由桌面壳接入受监督 PNG 导出与受管文件清理：条目包含不可变图像/代际引用、显示器与选区元数据、受管目录单文件名、SHA-256 内容摘要、OCR 状态和 tombstone 状态；索引 codec 有 magic/schema/长度/CRC 校验，保存使用同目录临时文件、`sync_all`、rename 与读取校验。只有通过 owner、generation 和截止时间门禁的 `SavePng` 完成事件才会写入历史；损坏索引启动时保留原文件并使用空内存索引，保存失败恢复本次内存插入。领域层按摘要和大小去重并按条数/字节配额将旧条目标记为 tombstone；清理器仅删除直属受管 PNG，在活动同名保护、删除失败或索引保存失败时保留 tombstone 供重试；041 的设置配额变更、042 的单条删除和 043 的全量清空复用相同的索引落盘与清理事务。042/043/049 提供历史预览、重新贴图、再次编辑、搜索与删除/清空；051 将受管 PNG 读取、摘要校验和解码移到单 worker，并按条目 generation、当前选择与意图拒绝陈旧结果；052 继续在该 worker 内按意图生成预览 XRGB、贴图 XRGB 或编辑 base/dimmed，历史完成分支不再进行这些整图转换。真实桌面探针尚未运行。
- `pinora-core::ocr` 与贴图窗口已于 2026-08-02 新增 `OcrTextSelection`：Ctrl+左键拖拽将物理窗口坐标映射为图像坐标，按相交词框和 OCR 阅读顺序生成局部文本，选中词框高亮；文本复制经既有 `ExportJobService` 监督并绑定 pin owner/asset，未通过真实 GUI/系统剪贴板探针。
- `pinora-app::FrameCache` 已于 2026-08-02 改为由单一状态锁维护预截帧、暂停标志和代际；缓存命中使用所有权移交而非克隆完整像素缓冲，暂停会清空槽位并使已在途抓取的发布失效。此事实由离线并发语义测试覆盖，不等价于真实桌面端到端延迟测量。
- `desktop_shell` 的 `PinWin` 已于 2026-08-02 缓存当前窗口尺寸与有效不透明度匹配的基础 XRGB 帧；OCR 词框、文本拖选和锁定边框的重绘只复制该帧并叠加装饰，不再重复整图缩放/压暗。resize、缩放和不透明度改变会使缓存失效；真实连续 resize、HiDPI、输入延迟和原生窗口性能尚未验证。
- 全屏截图入口已于 2026-08-02 接入：F3、托盘和 `CaptureFullDisplay` 会在当前捕获目标的完整图像上初始化有效 Overlay 选区；055 为多显示器 tray 增加带 `DisplayId` 的目标全屏项目，指定目标捕获前会重新枚举并拒绝失效 ID，缓存只在 ID、origin、尺寸和 scale 全部匹配时交付。区域模式不自动预选。多显示器联合画布、自动刷新显示器菜单、HiDPI 和真实窗口映射仍未验证。
- 056 在 tray 增加 1/3/5 秒延时区域截图与取消：开始时 `FrameCache` 暂停并清空，关闭 Pinora 的 Overlay/设置/历史，快照并只隐藏应用记录为可见的贴图；到期禁用缓存并走冷捕获，取得真实像素后恢复快照贴图再创建既有 Overlay。取消、捕获/加载/Overlay 失败和事件循环退出均恢复快照并保持 tray 驻留；没有新增倒计时窗口，所有既有 Overlay/贴图仍经过 `window_policy`。该事实只由离线状态测试和静态门禁支持，未验证原生任务栏、Dock、托盘或合成器时序。

## 2026-08-02 跨平台交付基线

- `pinora-app` 的 GTK 依赖已限制为 Linux target；Windows/macOS 不再在 `cargo check` 阶段探测 GTK/GLib 的 `pkg-config`。
- `OsSingleInstance` 在 Unix 保留 `instance.lock` + `activate.sock`；非 Unix 使用同目录文件锁和只绑定 `127.0.0.1` 的 loopback TCP，端口写入 `activate.port`。CLI 通过 `forward_ipc_frame` 统一转发。
- KWin 窗口放置在非 Linux 返回能力不可用；Linux desktop entry 在非 Linux 不创建。KDE/Spectacle 仍只在 Linux/KDE 会话探测，其他平台由 xcap 或 `Unavailable` 选择。
- `packaging/package-unix.sh` 生成 Linux raw binary、`.tar.gz`、可用时 `.deb`/`.rpm`；macOS 生成 raw binary、`.app` `.zip` `.dmg`。`package-windows.ps1` 生成 raw binary、`.zip`，检测到 NSIS 时额外生成 setup `.exe`。每个平台生成来源 `SHA256SUMS.txt`，release job 再生成覆盖全部上传资产的合并清单。
- `.github/workflows/ci.yml`、`package.yml`、`runtime-verify.yml` 已建立三平台原生 runner 矩阵；runtime smoke 只证明包可解包/安装和 `--version` 启动，不等价于 GUI、屏幕捕获、剪贴板、权限或多显示器验证。

2026-08-02 预发布交付证据：`v0.1.0-preview.6` 的 CI run `30720088334`、package run `30720098823`、发布 job `91422696939`、runtime-verify run `30720250257` 均成功；Release 含 Linux raw/tar/deb/rpm、macOS raw/zip/dmg、Windows raw/zip/setup 与合并 SHA256 清单共 11 个资产，下载后按 `SHA256SUMS.txt` 逐项复核通过。该证据只覆盖构建、分发、安装/卸载和 `--version` 启动探针，不扩展真实桌面能力声明。

## 主流程

统一 `desktop_shell` 事件循环（选区 + 贴图同一 loop，适配 Wayland）：

```text
启动 → 选区 Overlay → 松手出工具栏
  ├─ 选区内标注 / 工具栏：复制·贴图·保存·OCR·工具
  ├─ 双击复制 · 中键贴图 · Enter 贴图 · Esc 取消
  └─ 贴图窗：拖动·缩放·L 锁定·[ ]透明·O 再识别
```

## 构建与验证

- `cargo test --workspace`
- `cargo run`（图形会话）

## 尚未建立生产级证据的目标能力

- 选定并验证的 UI Adapter、完整工具条、OCR 富文本编辑器、跨屏联合 Overlay、系统主题/原生无障碍、历史全量清空/搜索/标签/再次编辑、真实透明/点击穿透，以及 Windows/macOS/X11/通用 Wayland 的正式适配与桌面验证。
- 复杂子孙进程回收和真实系统剪贴板验证仍未建立生产级证据；退出 worker 收敛只在 fake runner 上验证，尚无真实桌面探针。
