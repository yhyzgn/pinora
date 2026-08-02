# 任务 047：帧缓存零复制交接与暂停竞态修复

- 状态：已完成
- 计划：`.context/plans/047_frame_cache_handoff.md`
- 规模：中
- 依赖：`.context/tasks/020_capture_capability_truth.md`、`.context/tasks/031_overlay_annotation_asset_contract.md`
- 生产行为变更：是；只优化热键到 Overlay 的内部帧交接，保持截图内容与失败语义。

## 范围

- 将最新缓存帧从共享槽位移交给 Overlay，避免 `CachedFrame::clone` 复制全屏像素缓冲。
- 修复后台抓取与 `pause` 交错时的晚到写回。
- 为帧移交和暂停丢弃语义增加确定性测试。

## 任务目标

降低预截命中时的内存带宽和分配压力，使高分辨率 Overlay 首帧不再因缓存读取发生多份整帧复制。

## 非目标

- 不承诺或测量真实桌面 100–150ms 指标。
- 不替换 KDE/xcap 后端，不新增截图、窗口或 GPU 依赖。
- 不修改 Overlay 的渲染模型、标注行为或截图数据结构。

## 预期文件

- `crates/pinora-app/src/frame_cache.rs`
- `crates/pinora-app/src/desktop_shell.rs`
- `AGENTS.md`、`.context/plans/047_frame_cache_handoff.md`、`.context/tasks/047_frame_cache_handoff.md`
- `.context/system/overview.md`、`.context/system/risks.md`

## 验收标准

1. 缓存热命中把已有 `CachedFrame` 移交给调用方，不再克隆其图像与两个 XRGB 缓冲。
2. 暂停之后完成的后台抓取被丢弃，恢复前不会出现在缓存槽位。
3. 帧缓存新增回归测试，Overlay 缩放/标注资产契约和 workspace 门禁通过。

## 验证

- `cargo test -p pinora-app frame_cache::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：一次性移交后缓存短暂为空。缓解：Overlay 结束后恢复既有后台刷新；缓存未就绪仍走原 cold capture。
- 风险：暂停检查时机错误导致过度丢帧。缓解：只在抓取结束、发布前检查，恢复后下一轮正常抓取。
- 回滚：恢复 clone 读取和原发布逻辑；不影响历史或设置数据。

## 完成记录

- 2026-08-02：最新帧改为一次性所有权移交，热键检查只查询就绪状态，避免检查和读取阶段分别复制全屏像素缓冲。
- 2026-08-02：以暂停代际拒绝晚到帧；补充缓存移交、暂停后丢弃和重复恢复不打断抓取三项回归测试。
- 验证：`frame_cache::tests` 4/4、`desktop_shell::overlay_scale_tests` 9/9；workspace 115 app + 54 core 测试通过，2 个真实桌面测试忽略；fmt、check、严格 Clippy、diff 检查和 ctx validate 通过。
- 已知风险：这项工作不测量真实截图后端、窗口创建和合成器的时延，不能单独证明 Snipaste 级交互体验。
