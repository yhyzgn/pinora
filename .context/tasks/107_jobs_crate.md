# 任务 107：任务监督 crate

- 状态：已完成
- 计划：`.context/plans/107_jobs_crate.md`
- 规模：中
- 依赖：`pinora-core::JobSpec`/`JobResultRef`/`JobTerminalState`；现有 OCR、导出、历史加载应用服务。
- 生产行为变更：否；内部 crate 所有权迁移。

## 任务目标

建立 `pinora-jobs`，迁移 `job_supervisor` 与 `worker_lifecycle`，让 app 的具体任务服务依赖统一的取消、结果门禁和有界回收契约。

## 变更前记录

```text
目的：把通用任务生命周期从桌面应用实现中剥离，防止 OCR、导出和历史加载各自演化不同的取消/回收规则。
影响路径：Cargo workspace、app 模块导出、OCR/导出/历史读取/图像输出/桌面壳导入、任务模块源码和上下文文档。
兼容性：接口 / 数据 / 状态 / 租户 / 权限均不改变；保持 JobSpec、JobTicket、终态、取消和 timeout 语义。
外部副作用：无新增线程、进程、网络、系统注册或共享服务访问。
回滚点：恢复 pinora-app 的 job_supervisor/worker_lifecycle，移除 pinora-jobs。
验证场景：提交/取消、owner 关闭、超时、陈旧资产、worker 正常回收、期限后残留统计和全部调用方。
```

## 范围

- 新增 `crates/pinora-jobs/{Cargo.toml,src/{lib,job_supervisor,worker_lifecycle}.rs}`。
- 更新 workspace 与 `pinora-app` 依赖、兼容 re-export、全部任务消费者的导入。
- 删除 app 内同名模块，更新设计文档、系统事实、风险和验证记录。

## 非目标

- 不拆 OCR、导出、历史加载、编码、系统剪贴板或 UI。
- 不修改任何后台任务的业务实现、超时值、缓存、文件格式或用户可见反馈。

## 预期文件

- `Cargo.toml`、`Cargo.lock`
- `crates/pinora-jobs/Cargo.toml`、`crates/pinora-jobs/src/{lib,job_supervisor,worker_lifecycle}.rs`
- `crates/pinora-app/Cargo.toml`、`crates/pinora-app/src/{lib,desktop_shell,export_job,history_load_job,image_sink,ocr,ocr_job}.rs`
- `AGENTS.md`、`.context/{plans,tasks}/107_jobs_crate.md`
- `docs/Pinora-开发设计文档.md`、`.context/system/{overview,conventions,risks}.md`

## 验收标准

1. `pinora-jobs` 仅依赖 `pinora-core`，拥有迁移后的监督与回收测试。
2. app 不再声明或编译旧模块，但公开的任务类型兼容 re-export 仍有效。
3. 所有具体服务保持已有取消、超时、owner、资产 generation 和 worker 收敛测试。
4. 严格 workspace 门禁、Windows target 和 ctx 校验通过。

## 验证

- `cargo test -p pinora-jobs -- --nocapture`
- `cargo test -p pinora-app --lib job_supervisor -- --nocapture`
- `cargo test -p pinora-app --lib worker_lifecycle -- --nocapture`
- `cargo test -p pinora-app --lib ocr_job -- --nocapture`
- `cargo test -p pinora-app --lib export_job -- --nocapture`
- `cargo test -p pinora-app --lib history_load_job -- --nocapture`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --target x86_64-pc-windows-msvc`
- `cargo fmt --check`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：`pub(crate)` 回收类型变为跨 crate API 时出现过度暴露，或调用方遗漏导致生命周期路径分叉。
- 回滚：恢复 app 内实现和导入，移除新 crate；用户数据与后台行为保持不变。

## 完成记录

- 已新增 `pinora-jobs`，迁移通用监督器和 worker 生命周期工具；app 已移除旧模块并通过 `pub use pinora_jobs` 保持兼容导出。
- 已更新 OCR、导出、历史加载、图像输出、OCR 适配和桌面壳的导入，具体服务继续拥有其线程/子进程和业务逻辑。
- 已通过 `cargo test -p pinora-jobs -- --nocapture`（7 通过）、OCR（13）、导出（12）、历史加载（10）定向测试，以及 `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`（根 1、app 254、capture 25、core 90、jobs 7、platform 21 通过；app/capture 各 1 个真实桌面测试忽略）、workspace check、严格 Clippy、Windows target、fmt、diff 和 `ctx validate`。
- 真实桌面退出、协作式子进程取消和性能仍是开放风险。
