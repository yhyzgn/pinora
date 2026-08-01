# 系统全景：pinora

## 技术与运行基线

- Rust 2024 workspace：`pinora`（`src/main.rs`）+ `pinora-core` + `pinora-app`。
- 依赖：`ctrlc`、`fs2`、`png`、`xcap`、`winit`、`softbuffer`、`fontdue`（标注文本）、`tray-icon`/`gtk`（托盘）。
- Linux xcap 需 `pipewire-devel`、`mesa-libgbm-devel`（**仅 xcap/portal 兜底路径**）。
- **当前截图后端（Linux/KDE 实验路径）**：`kde-spectacle`（KWin，~0.5s）→ `xcap`/portal（慢）→ 受限能力状态；`FakeCaptureProvider` 仅由显式测试/开发注入使用，不能是生产截图成功的降级结果。
- **不要默认 portal**：portal/PipeWire 是通用 Wayland 兜底，不是 Snipaste 级体验。
- **全局热键**：`global-hotkey`（F2/Ctrl+N/Ctrl+Shift+S）+ 单实例 IPC `pinora capture`；启动时写入 `~/.local/share/applications/pinora.desktop`。
- **系统剪贴板**：Linux 优先 `wl-copy`，回退 `xclip`；当前仍保留内存副本，适配器已直接持有子进程并在截止时间后回收，但系统写入失败与内存成功的双结果语义仍需后续修正。

## 当前可运行的实验能力（未达到生产声明）

| 能力 | 说明 |
| --- | --- |
| 截屏尝试 | KDE 优先 spectacle/KWin；否则 xcap；两者不可用时返回 `CapabilityUnavailable`，不生成 fake 图像 |
| 区域选区 Overlay | 拖选后工具栏；双击复制、中键/Enter 贴图；选区内标注/OCR |
| 贴图窗口 | 无边框置顶、拖动、滚轮缩放、Esc 关闭；多贴图 |
| 导出 | PNG 文件 + 内存剪贴板 + 系统剪贴板（wl-copy/xclip） |
| 全局热键 | F2/Ctrl+N/Ctrl+Shift+S + `pinora capture` IPC |
| 单实例 | flock + Unix socket Activate/CAPTURE/QUIT |
| 帧缓存 | 空闲预截，overlay 瞬时弹出 |
| 基础标注 | Overlay 选区内：矩形/箭头/画笔/椭圆/马赛克/文本；C 颜色；+/- 线宽 |
| 系统托盘 | 截图 / 退出（tray-icon） |
| 贴图控制 | L 锁定，`[` `]` 透明度（压暗近似）；`O` 本地 OCR；`T` 词框 |
| OCR | 系统 `tesseract` CLI；全文复制剪贴板；词框叠加；缺引擎可降级提示 |

## 2026-08-01 接管审计事实

- 生产入口仍是 `src/main.rs`，但使用 `std::os::unix::net::UnixStream`；没有平台条件编译，因此 Windows target 无法完成 workspace 检查。
- `crates/pinora-app/src/desktop_shell.rs` 为 2461 行，集中承载 GPUI/窗口事件、截图编排、Overlay 绘制、标注输入、贴图生命周期、OCR 触发、托盘和 IPC 轮询；最近连续提交 `eb5dfaf`、`e0ea849`、`3912f64`、`f3cb45a`、`b0bd260` 均修补该文件的交互/性能问题。
- 当前依赖树把 `gtk`/`tray-icon`、`xcap`/PipeWire、`winit`/`softbuffer` 和 Linux CLI 后端直接放入 `pinora-app`；没有 Windows/macOS/Linux 适配器边界。
- `cargo fmt --check`、`cargo check --workspace` 和 `cargo clippy --workspace --all-targets -- -D warnings` 已于 2026-08-01 通过；`cargo test --workspace` 通过 113 个可执行单元测试（74 app、39 core），另有 2 个真实桌面测试被忽略；仍没有 GUI 端到端测试。
- `cargo check --workspace --target x86_64-pc-windows-msvc` 失败于 GTK 的 `gdk-pixbuf-sys`/`glib-sys` pkg-config 交叉编译，尚未进入应用代码编译阶段。
- OCR 通过 `tesseract` 子进程和临时 PNG 工作；适配器已持有自身 `Child`，支持协作式取消、30 秒截止时间、16 MiB 输出上限和 RAII 临时文件清理，不再调用外部 `kill`。贴图与 Overlay UI 已经通过 `OcrJobService` 提交到 `JobSupervisor`，结果交付受 owner、终态和 `AssetRef` generation 门禁保护；worker 不触碰窗口或剪贴板。
- 截图后端自动选择 KDE `spectacle` → xcap → `Unavailable`；两者不可用时保留后端失败摘要并由 provider 返回 `CapabilityUnavailable`，`fake` 只能通过显式测试/开发注入使用。
- `docs/Pinora-开发设计文档.md` 已于 2026-08-01 更新为 v1.0 生产重构基线：明确当前实验实现、目标端口/适配器架构和待验证技术决策；文档不代表任何新功能已经交付。
- `pinora-core::asset` 已于 2026-08-01 新增 `AssetGeneration` 和 `AssetRef` 领域契约；它只组合既有 `ImageId`，可判定陈旧结果，已用于桌面贴图和 Overlay OCR 结果门禁，导出路径尚未接入。
- `pinora-core::job` 与 `pinora-app::JobSupervisor` 已于 2026-08-01 新增：任务元数据绑定 `JobId`、关联 ID、`AssetRef`、领域 owner、类型和截止时间；监督器可协作式取消、关闭 owner、标记超时并拒绝终态或陈旧版本结果。桌面 OCR 已接入，导出/剪贴板仍未接入，不能据此声称所有后台进程已受控回收。
- `pinora-app::OcrJobService` 已于 2026-08-01 接入 `desktop_shell`：可注入 runner 在 worker 中执行 OCR，主线程轮询通过 `JobSupervisor` 后才交付结果，覆盖失败、owner 关闭、超时和 generation 失效。贴图关闭、Overlay 取消/再截和应用退出均会取消对应任务；服务契约测试仍不等价于真实窗口 E2E。
- `pinora-app::image_sink` 已于 2026-08-01 收敛系统剪贴板子进程：输入和 stderr 使用 RAII 临时文件，适配器直接轮询 `Child`，超时只对拥有的 child 执行 `kill`/`wait`；其图像/文本复制入口已由桌面 `ExportJobService` 调用。
- `pinora-app::ExportJobService` 已于 2026-08-01 接入 `desktop_shell`：统一监督 PNG 保存、图像剪贴板和 OCR 文本剪贴板输入，主循环按 owner、job ID、资产 generation、截止时间和终态门禁结果；服务契约与纯逻辑测试仍不等价于真实窗口 E2E。
- `pinora-app::save_png_file` 已于 2026-08-01 使用同目录临时文件、文件 `sync_all`、rename 发布和目标可读性校验；未提交临时文件由 RAII 删除。该事实只在 Linux 本地文件系统测试，未证明跨平台覆盖或断电后目录持久性。
- `OcrJobService` 与 `ExportJobService` 已于 2026-08-01 保存自己创建的 worker 句柄，正常轮询会回收结束线程；桌面退出先取消、最多等待 2 秒并输出取消/join/panic/残留计数。协作式 worker 若不响应取消会被如实报告为残留，不能视为已回收。

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

- 选定并验证的 UI Adapter、完整工具条、OCR 拖选编辑器、跨屏联合 Overlay、设置持久化、真实透明/点击穿透，以及 Windows/macOS/X11/通用 Wayland 的正式适配与桌面验证。
- 复杂子孙进程回收和真实系统剪贴板验证仍未建立生产级证据；退出 worker 收敛只在 fake runner 上验证，尚无真实桌面探针。
