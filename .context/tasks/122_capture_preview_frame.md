# 任务 122：捕获预览帧数据契约

- 状态：已完成
- 计划：`.context/plans/122_capture_preview_frame.md`
- 规模：中
- 依赖：任务 106、119、120、121 已完成。
- 生产行为变更：否；内部预览帧数据所有权迁移。

## 任务目标

让 `pinora-capture` 唯一拥有 Overlay 预览帧的像素转换、完整性校验和缓存帧移交，令 app 只编排捕获结果、窗口与 Surface。

## 变更前记录

```text
目的：统一冷捕获和预截帧的预览像素数据契约，删除 desktop_shell 重复转换/校验逻辑。
影响路径：冷捕获结果通道、预截帧命中、Overlay 打开前的预览缓冲完整性检查。
兼容性：不改变接口、数据、状态、租户或权限语义。
外部副作用：无；不创建窗口、不访问文件、不启动新增线程、不连接外部基础设施。
回滚点：恢复 desktop_shell 的 PreparedPreview 与辅助函数，移除 capture crate 对应导出。
验证场景：RGBA 转换、完整/短缓冲、CachedFrame 所有权移交、app 冷捕获回归。
```

## 范围

- 在 `pinora-capture` 定义并导出预览值对象。
- 收拢从 `CaptureImage` 构建预览、从 `CachedFrame` 移交预览和完整性检查。
- 切换 app 的冷捕获与缓存命中路径，删除本地重复类型和函数。
- 为 capture crate 增加目标契约测试。
- 更新 crate 导出、设计/系统/风险文档。

## 非目标

- 不修改 `CaptureProvider`、截图后端、`FrameCache` 刷新调度、cold capture 线程、接收通道机制、`OverlayState`、Window/Surface、softbuffer、输入、托盘或 EventLoop。
- 不改变 RGBA 转 XRGB、暗化、错误码、捕获目标或用户可见结果。

## 预期文件

- `AGENTS.md`
- `.context/plans/122_capture_preview_frame.md`
- `.context/tasks/122_capture_preview_frame.md`
- `crates/pinora-capture/src/{lib,capture_preview,frame_cache}.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `docs/Pinora-开发设计文档.md`
- `.context/system/{overview,conventions,risks}.md`

## 验收标准

1. capture crate 唯一拥有预览帧的构造、缓存帧移交与缓冲完整性校验；app 删除 `PreparedPreview` 及本地辅助函数。
2. 完整和短 XRGB 缓冲、RGBA 转换及缓存帧移交都由 capture 测试覆盖，且 crate 依赖方向不变。
3. app 仍独占捕获编排、错误处理、Overlay 目标、Window/Surface、softbuffer present 和唯一 EventLoop。

## 验证

- `cargo test -p pinora-capture -- --nocapture`
- `cargo test -p pinora-app --lib -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：预览所有权或缓冲检查变化可能造成无效帧、重复分配、错误回退或 Overlay 首帧像素回归。
- 回滚：恢复 app 内预览类型、转换和校验，移除 capture crate 导出；不触碰截图后端、窗口、输入、OCR、导出、历史、托盘或数据格式。

## 完成记录

- 2026-08-03：`CapturePreview` 接管 RGBA 到 XRGB 基础/暗化帧生成、由已有 worker 组装的预览值与物理像素长度检查；`CachedFrame::into_preview_with_display` 以移动语义交付预览和显示器信息。app 已删除 `PreparedPreview`、`prepare_preview` 和 `preview_buffers_match_image`，仍独占线程调度、错误映射、Overlay 目标、Window/Surface、softbuffer present 与唯一 EventLoop。
- 测试覆盖：预览 RGBA 转换、完整缓冲、短缓冲拒绝、缓存帧像素所有权移交、app 捕获与 Overlay 回归。
- 完成验证：`cargo test -p pinora-capture -- --nocapture`（27 通过、1 项真实显示会话测试按既有约定忽略）、`cargo test -p pinora-app --lib -- --nocapture`（35 通过）、`cargo run --quiet -- --version`（输出 `pinora 0.1.0`）、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（capture/export 各 1 项真实桌面测试按既有约定忽略）、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo check --workspace --target x86_64-pc-windows-msvc`、`cargo fmt --check`、`git diff --check`、`python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`。
- 未覆盖风险：离线契约、交叉编译和版本探针不能证明真实截图权限、缓存时序、GUI、softbuffer、HiDPI、任务栏/Dock、托盘常驻或性能；详见 R-073。
