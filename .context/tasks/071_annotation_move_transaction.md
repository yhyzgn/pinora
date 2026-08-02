# 任务 071：Overlay 标注移动、键盘微调与可撤销事务

- 状态：已完成
- 计划：`.context/plans/071_annotation_move_transaction.md`
- 规模：中
- 依赖：`.context/tasks/030_annotation_revision_contract.md`、`.context/tasks/068_overlay_preview_cache.md`、`.context/tasks/069_clear_annotation_transaction.md`、`.context/tasks/070_annotation_selection_delete.md`
- 生产行为变更：是；当前 Overlay 中选中的标注可拖动或以方向键移动。

## 任务目标

在不增加窗口或后台任务的条件下，让当前 Overlay 的选中标注可实时预览移动，并通过一次释放或一次方向键产生一个可撤销事务；移动期间不改变导出、贴图、OCR 或已提交缓存输入。

## 范围

- 为 `Annotation` 与 `AnnotationDoc` 增加平移和原位替换事务。
- 为 Overlay 增加选中对象拖动、瞬态预览、1/10 像素键盘移动和生命周期失效。
- 覆盖各对象的几何保持、事务顺序、预览隔离、缓存/资产、键盘和窗口策略回归。
- 更新计划、任务、系统事实和风险记录。

## 非目标

- 不实现缩放、旋转、锚点、样式编辑、多选、对齐、排序、持久化或贴图独立编辑。
- 不改捕获、贴图、OCR、导出、历史、系统菜单、窗口策略或后台任务。

## 预期文件

- `crates/pinora-core/src/{annotate.rs,lib.rs}`
- `crates/pinora-app/src/{desktop_shell.rs,overlay_preview_cache.rs}`
- `AGENTS.md`
- `.context/plans/071_annotation_move_transaction.md`
- `.context/tasks/071_annotation_move_transaction.md`
- `.context/system/{overview.md,risks.md}`

## 验收标准

1. 各类型标注平移后保留全部视觉字段；一次有效拖动/方向键可 undo/redo，零位移和取消不改变 revision。
2. 拖动预览与提交后合成逐字节一致，且预览不修改文档、导出、贴图或 OCR 输入；提交才更新 asset generation 和已提交缓存。
3. 有选中对象时方向键移动对象（Shift 为 10 像素），无选中对象保留既有选区移动。
4. 不创建窗口、事件循环、系统菜单、截图或 worker；Pinora 空闲只在 tray，Overlay/贴图/辅助层禁止出现任务栏、Dock 或分页器项。

## 验证

- `cargo test -p pinora-core annotate -- --nocapture`
- `cargo test -p pinora-app overlay_preview_cache::tests -- --nocapture`
- `cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`
- `cargo test -p pinora-app window_policy::tests -- --nocapture`
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`
- `git diff --check`
- `python /home/neo/.agents/skills/ctx/scripts/context_bootstrap.py validate --root /home/neo/Projects/neo/pub/pinora`

## 风险与回滚

- 风险：替换事务破坏坐标或 redo 顺序。缓解：保存索引/旧对象/新对象并覆盖全部对象类型与删除/清空交错。
- 风险：预览污染已提交层或马赛克/模糊源。缓解：使用原始裁剪生成独立呈现缓冲，并做提交前后像素比较。
- 风险：高频拖拽造成重绘延迟或窗口策略绕过。缓解：复用现有缓存/节流和 Overlay，不加窗口/worker，运行 `window_policy` 守卫。
- 回滚：移除平移替换事务和拖拽预览；选择、删除、清空、导出、tray 与窗口策略保持。

## 完成记录

- 已完成：新增 `Annotation::translated` 和索引原位 `Replace` 事务；各对象的颜色、线宽、填充、圆角半径、序号、文本和马赛克/模糊参数保持不变。有效替换、删除、清空、undo/redo 的交错顺序由核心测试锁定；零位移和无效索引不推进 revision 或清除 redo。
- 已完成：Overlay 选择拖拽使用独立 `SelectedAnnotationDrag`，鼠标移动只更新瞬态 preview，释放时才提交一次替换。替换预览缓存从不可变原始裁剪重建前缀、叠加移动对象及后续对象；马赛克和模糊不读取已提交像素，预览与提交后烧录逐字节一致。
- 已完成：方向键优先移动选中对象，Shift 为 10 像素；无选中对象保持原有选区移动。拖动可越过选区边界，Esc、重选、工具切换、提交、undo/redo、删除与清空均清理瞬态拖拽。
- 已验证：`cargo test -p pinora-core annotate -- --nocapture`、`cargo test -p pinora-app overlay_preview_cache::tests -- --nocapture`、`cargo test -p pinora-app desktop_shell::overlay_scale_tests -- --nocapture`、`cargo test -p pinora-app window_policy::tests -- --nocapture`、`cargo fmt --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`PINORA_NO_SYSTEM_CLIPBOARD=1 cargo test --workspace`、`git diff --check` 与 `ctx validate` 通过；全量离线结果为 app 176 通过、2 忽略，core 81 通过。
- 未验证：离线测试不能证明真实 Windows、macOS、X11、KDE Wayland 中的高 DPI 命中和拖拽、帧时间、任务栏/Dock/分页器隔离或 tray 连续驻留；这些仍需原生会话验收。
