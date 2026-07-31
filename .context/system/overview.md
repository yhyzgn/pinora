# 系统全景：pinora

## 技术与运行基线

- Rust 2024 workspace：`pinora`（`src/main.rs`）+ `pinora-core` + `pinora-app`。
- 依赖：`ctrlc`、`fs2`、`png`、`xcap`、`winit`、`softbuffer`、`fontdue`（标注文本）、`tray-icon`/`gtk`（托盘）。
- Linux xcap 需 `pipewire-devel`、`mesa-libgbm-devel`（**仅 xcap/portal 兜底路径**）。
- **截图后端优先级（KDE Wayland）**：`kde-spectacle`（KWin，~0.5s）→ `xcap`/portal（慢）→ `fake`。
- **不要默认 portal**：portal/PipeWire 是通用 Wayland 兜底，不是 Snipaste 级体验。
- **全局热键**：`global-hotkey`（F2/Ctrl+N/Ctrl+Shift+S）+ 单实例 IPC `pinora capture`；启动时写入 `~/.local/share/applications/pinora.desktop`。
- **系统剪贴板**：Linux 优先 `wl-copy`，回退 `xclip`；另保留内存副本。

## 已实现能力

| 能力 | 说明 |
| --- | --- |
| 真实/降级截屏 | KDE 优先 spectacle/KWin；否则 xcap；再 fake |
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

## 未实现

- GPUI/Liora、完整 GUI 工具条、OCR 拖选编辑器、跨屏联合 Overlay、设置持久化、真透明。
