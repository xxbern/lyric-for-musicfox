# Stage-3 全面 Code Review 总览

> Reviewer: Code Agent（静态评审，未跑 cargo test）
> Scope: `git diff --stat HEAD` 列出的 30 个文件 + 新增 `src/services/wt.rs`
> 上游文档：`docs/dev-stage-3/req.md`、`docs/dev-stage-3/dev.md`、`docs/dev-stage-3/P0-P3-bug.md`
> 对比阶段：stage-2 → stage-3（Slint 重构 + P0~P4 托盘 / 重载 / UI / WT 托管 / go-musicfox 卡片）

## 1. 评审方法与依据

1. **逐文件对照需求**：把 `req.md` / `dev.md` / `P0-P3-bug.md` 中的每一项验收点拆成"代码能/不能兑现"，形成 PASS / FAIL / MISSING 矩阵。
2. **静态代码扫描**：
   - 死代码 / 未引用模块（`mod` 声明但 `grep` 无任何引用）；
   - 公开 trait 与实现之间契约对齐（`Platform` 各子 trait 的实现完整性）；
   - 资源/线程所有权（`OnceLock<Mutex<…>>` 静态状态、thread::spawn 是否 panic 路径释放）；
   - Slint ↔ Rust 双向同步闭包（`Rc<RefCell<…>>` 跨 `Slint::Timer` + 回调）；
   - 错误处理与用户感知（toast 文案、关闭窗口校验、并发竞态）。

3. **不**重新跑 `cargo test`、不修改代码；评审输出统一放在 `docs/dev-stage-3/`。

## 2. 结论速查

| 维度 | 评级 | 一句话 |
|---|---|---|
| 配置序列化与默认 / 选项 | **PASS** | `src/config/mod.rs` + `tests/config_test.rs` 完整覆盖；`[wt]` 节缺失回退 OK |
| 路径解析（go-musicfox DataDir） | **PASS** | `src/path.rs::resolve_musicfox_data_dir()` 与 req §6.3 优先级一致 |
| 托盘 / 重载交互（P0/P1） | **FAIL** | 设置 UI 没有"重载歌词"按钮；`notify_reload()` 仍然是 unit 返回 |
| 设置 UI 现代化（P1） | **PARTIAL** | 卡片化 / 端口只读 / 预览 已落地；`ScrollView` 未移除（违反 BUG-04 修复 #2） |
| 实时预览（P2） | **PASS** | 8 向偏移 + `#d4dce7` 背景 + `天青色等烟雨` + 不进脏标记，行为符合 req §4 |
| WT 托管（P3） | **PARTIAL** | 主干逻辑完整；细节存在多处串户（参见 `review-finding.md` §3） |
| go-musicfox 设置卡片（P4） | **PASS** | 路径 + 浏览 + DataDir 按钮 + 校验 + 落盘均完整 |
| 模块化 / 平台抽象 / 死代码 | **FAIL** | 三处重复 `TrayCmd` 定义；`src/tray/windows_tray.rs`、`src/tray/stub.rs`、`src/services/tray.rs` 均无引用 |
| 错误反馈一致性（F10） | **FAIL** | req §2 的"已发送重载通知 / 主进程未运行"差异化 toast 路径不存在 |
| 文档同步 | **FAIL** | `dev.md` 与实际实现多处不一致（接口签名 / 行为细节） |

## 3. 详评入口

| 文档 | 内容 |
|---|---|
| [`review-requirements-matrix.md`](./review-requirements-matrix.md) | req.md §1 ~ §8 全部验收点的 PASS / FAIL / MISSING 矩阵 |
| [`review-finding.md`](./review-finding.md) | 按严重等级罗列的代码缺陷 / 死代码 / 资源泄漏 / 跨线程问题 / 文档漂移 |
| [`review-architecture.md`](./review-architecture.md) | 平台抽象 / 服务分层 / Slint↔Rust 同步模型 / 并发模型评估 |
| [`review-recommendations.md`](./review-recommendations.md) | 修复优先级建议（不分 PR 一次性提交） |

## 4. 风险提示（先看这里）

1. **F10 重载反馈不可达**：当前实现里设置进程点击"保存"后，Rust 直接调 `restart_lyric_app()`（杀旧进程 + 拉新进程）。这意味着 req §2.2 / §2.3 描述的"发送 reload 通知 → 主进程在线/离线差异化 toast"这条用户旅程**当前代码根本走不到**。`tests/settings_test.rs::toast_lifecycle_visible_and_expires` 仅断言字符串字面量，没有覆盖这条用户故事。
2. **设置 UI 仍有 `ScrollView`**：`ui/settings.slint:130` 的 `ScrollView` 还在。`P0-P3-bug.md` §3 第 2 项明确要求"移除外层 `ScrollView`，改为自然流式布局，保证 100% 无滚动条"，实际并未执行。
3. **死模块**：三份 `TrayCmd` 定义共存；`tray::windows_tray` / `tray::stub` 没有 `mod` 引用，`services::tray` 没有调用点。建议下次重构时统一移除。
4. **`ShowContextMenu` 在 UI 线程同步阻塞**：`src/window/mod.rs:406` 收到 `TrayCmd::ShowContextMenu` 后，在 Slint tick 中同步调用 `TrackPopupMenuEx`（模态、阻塞 100ms 量级）。Slint 渲染循环会被阻塞——大菜单或慢系统下会出现肉眼可见卡顿。建议将弹窗下沉到独立 OS 线程或异步队列。
5. **菜单底部仍可能溢出屏幕**：`src/platform/windows/mod.rs:240` 仅用 `TPM_TOPALIGN` 把菜单顶部对齐到鼠标 Y，但**没有**根据菜单高度 + 屏幕工作区做向上位移。req §1.4 要求"顶部 ≤ 鼠标 Y 且 底部 ≤ 屏幕高"，现行实现只满足前者。详细见 `review-finding.md` §3.2。
6. **`MINIMIZE_START` 拦截可能误伤**：钩子是全局进程范围监听（`SetWinEventHook` PID=0, thread=0），按窗口标题包含匹配。Windows Terminal 自身内部的 `TerminalWindow.ico` 标题里也可能包含 `MusicFoxTerminal` 字符串（取决于 user 配置），需确认 wt 启动时只一个窗口被托管。
7. **`once_cell::OnceLock` 内部状态无重置**：`WT_HOOK` / `WT_TARGET_TITLE` / `LAST_TOGGLE` / `IS_LAUNCHING` 都是进程级静态，**多次启动 WT 切换 title 后不会清旧 hook**，需要 `shutdown_wt` 一定被调用。