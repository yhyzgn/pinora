# 系统全景：pinora

## 技术与运行基线

- Rust 2024 workspace：`pinora`（`src/main.rs`）+ `pinora-core` + `pinora-app`。
- 依赖：`ctrlc`、`fs2`、`png`、`xcap`、`winit`、`softbuffer`。
- Linux xcap 需 `pipewire-devel`、`mesa-libgbm-devel`。

## 已实现能力

| 能力 | 说明 |
| --- | --- |
| 真实/降级截屏 | xcap 优先，失败 fake |
| 区域选区 Overlay | 拖拽、Enter/Esc、方向键；脏矩形优化 |
| 贴图窗口 | 无边框置顶、拖动、滚轮缩放、Esc 关闭；多贴图 |
| 导出 | PNG + 内存剪贴板 |
| 单实例 | flock + Unix socket Activate |

## 主流程

统一 `desktop_shell` 事件循环（选区 + 贴图同一 loop，适配 Wayland）：

```text
启动 → 选区 Overlay → 裁剪 → 贴图窗口（置顶）
  ├─ 左键拖动（Wayland: drag_window 协议）
  ├─ 滚轮缩放 · Esc 关闭贴图
  ├─ F2 / Ctrl+N 再截
  └─ Ctrl+Q 退出
```

## 构建与验证

- `cargo test --workspace`
- `cargo run`（图形会话）

## 未实现

- GPUI/Liora、标注、全局热键、系统剪贴板、跨屏联合 Overlay、托盘。
