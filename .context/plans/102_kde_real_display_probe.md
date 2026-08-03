# 计划 102：KDE 真实显示器探测

- 状态：已完成
- 负责人：Codex
- 当前任务：`.context/tasks/102_kde_real_display_probe.md`

## 目标

移除 KDE 捕获后端在 `kscreen-doctor` 失败时返回固定虚拟显示器的伪拓扑，改为解析真实 `xrandr --query` 输出；两种探测均失败时进入受控不可用状态。

## 非目标

- 不改变 Spectacle 捕获命令、区域/全屏裁剪、AllDisplays、窗口捕获、热键、tray 或设置格式。
- 不把 xrandr 输出当作 Wayland 通用支持；Wayland 合成器没有真实拓扑时必须明确降级。
- 不新增 fake 运行时回退、窗口、线程、依赖或外部联网。

## 约束

- 显示器 ID、bounds、连接状态和尺寸必须来自成功的系统探测；禁止硬编码分辨率。
- 解析失败、命令失败或没有已连接输出必须返回 `CapabilityUnavailable`，不暴露半成品列表。
- 所有外部命令错误只进入现有受控错误路径，不把原始 stdout/stderr 写入用户可见反馈。

## 依赖关系

- 依赖现有 `DisplayInfo`、`KdeSpectacleCaptureProvider::displays` 和 `CaptureRequest` 坐标模型。
- 依赖系统 `xrandr --query` 在 X11 会话可用；Wayland 仍优先 `kscreen-doctor`，否则受控降级。

## 检查点

1. `kscreen-doctor` 成功时保持现有解析结果。
2. `kscreen-doctor` 失败时只接受有效 xrandr connected 输出，解析连接名、primary 标记、尺寸和正/负坐标。
3. xrandr 无输出、命令失败或 malformed geometry 均不产生伪显示器。

## 阶段

1. 实现真实 xrandr 探测和纯文本解析。
2. 覆盖多显示器、负坐标、断开输出和 malformed 输入测试。
3. 运行 workspace/跨 target/上下文门禁，提交并推送。

## 计划级风险

- 某些 KDE/Wayland 环境没有 xrandr，且 kscreen backend 也不可用；结果应是明确的截图能力不可用，而不是伪造显示器。
- xrandr 输出格式在驱动版本间有细微差异；解析器需保持严格尺寸校验，并将未知格式记录为风险而非猜测。

## 完成标准

- 生产代码不再返回固定分辨率或虚拟占位显示器；真实探测失败可被诊断且不会创建错误截图资产。
- 定向测试、workspace check、严格 Clippy、全量测试、Windows target、`ctx validate` 和 `git diff --check` 通过。

## 风险与回滚

- 若 xrandr 解析覆盖不足，回滚为禁用 KDE 后端并保留 xcap/IPC/tray，不恢复伪拓扑。
- 真实 KDE 多屏/Wayland 探针仍需单独记录，不以解析单测代替。

## 完成记录

- 已移除固定 `3840x2160` 虚拟显示器回退；`kscreen-doctor` 失败时执行真实 `xrandr --query`，严格解析 connected 输出、primary 标记、正负坐标和物理尺寸。
- xrandr 命令失败、无 connected 输出或 geometry 无效时返回 `CapabilityUnavailable`，不创建显示器条目、不生成错误截图资产；Spectacle、区域/全屏裁剪和其他平台路径保持不变。
- 已验证 `capture_kde` 8 项定向测试、workspace check、严格 Clippy、Windows target、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（app 302 通过、2 忽略；core 90 通过）、格式、`ctx validate` 与 `git diff --check`。真实 KDE/X11/Wayland 探测兼容性继续由 R-061 跟踪。
