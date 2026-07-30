# 计划 006：截图抽象与 OS 单实例

- 状态：已完成
- 负责人：未分配
- 当前任务：`.context/tasks/006_capture_os_instance.md`

## 目标

1. 引入 `CaptureProvider` 与 `CaptureRequest`，用 fake 实现可测区域捕获并接入命令分发。  
2. 实现基于文件锁 + Unix socket 的 OS 单实例：二次启动向主实例转发 Activate。

## 非目标

- 真实 xcap/Portal 截图、GUI Overlay。  
- Windows/macOS 单实例后端（本阶段实现 Unix 路径，接口可扩展）。  
- 完整 IPC 命令面（仅 Activate）。

## 约束

- `CaptureProvider` 对 core 可表达；fake 与 OS 适配在 `pinora-app`。  
- 单实例路径使用可写 runtime/temp 目录；测试使用隔离临时目录。  
- 不连接共享外部服务。

## 依赖关系

- 依赖计划 003–005。

## 阶段

1. core 捕获类型与命令/事件。  
2. FakeCaptureProvider + Runtime 捕获分发。  
3. OsSingleInstance（flock + socket）与 main 接入。  
4. 验证与上下文同步。

## 退出标准

- 单元测试覆盖 fake 捕获与双实例转发。  
- 主进程 `cargo run` 经 Capture 创建贴图；二次 `cargo run` 退出并激活主实例计数。

## 检查点

- 测试不依赖真实显示器权限。

## 计划级风险

- Unix socket 路径权限/残留：release 时清理；测试用唯一目录。

## 完成标准

- 离线可测的捕获协议 + 可用的 OS 单实例主路径。
