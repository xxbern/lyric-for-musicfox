# Stage-3 架构评审

> 关注点：平台抽象边界、Slint↔Rust 同步、并发模型、模块职责、资源生命周期。
> 与 `review-finding.md` 互补：finding 讲"哪里坏了"，architecture 讲"为什么这么组织"以及"组织上的不变量是什么"。

## 1. 模块拓扑（stage-3 变更后）

```
src/
├── main.rs                  ← 进程入口：CLI / mutex / config load / GUI
├── lib.rs                   ← slint::include_modules + 公共 re-exports
├── bin/mock_udp_sender.rs   ← 测试工具（独立二进制）
├── cli.rs
├── config/
│   ├── mod.rs               ← Config / WindowConfig / LyricStyleConfig / SystemConfig / WtConfig
│   ├── load.rs              ← load() (from toml)
│   └── save.rs              ← save() (atomic write)
├── context/
│   └── AppContext           ← Arc<内部状态>：config RwLock + signals Atomics + event_bus
├── event_bus/
│   └── AppEvent             ← RequestRepaint / ConfigReloaded / TrayCmd / LyricStateChanged
├── instance/                ← acquire_main_mutex / acquire_settings_mutex 的跨平台 shim
├── path.rs                  ← app_data_dir / config_path / resolve_musicfox_data_dir
├── pipe/
│   ├── mod.rs               ← reload / pos / presence 三个 IPC 命名空间
├── platform/
│   ├── mod.rs               ← Platform trait + current() 单例
│   ├── trait.rs             ← 8 个子 trait：SessionId / Instance / Tray / PipeServer / PipeClient / WindowStyle / Shell / Monitor / Wt
│   ├── windows/mod.rs       ← WindowsBackend：~1000 行（pipe / tray / wt / shell 全在内）
│   └── stub/mod.rs          ← StubBackend：纯 no-op
├── protocol/
│   └── IpcMessage / get_session_id / platform_pipe_path
├── services/
│   ├── mod.rs               ← ServiceHandles 聚合（8 个 service）
│   ├── config.rs / font.rs / monitor.rs / position.rs / style.rs / tray.rs / udp.rs / wt.rs
│   └── (wt.rs NEW in stage-3)
├── settings/
│   ├── mod.rs               ← Slint `SettingsWindow` 入口
│   ├── form.rs              ← FormState（draft / dirty / 防抖 / toast）
│   └── validate.rs
├── tray/
│   ├── mod.rs               ← 实际使用的 TrayCmd / init_tray / update_flash
│   ├── windows_tray.rs      ← 旧实现（dead code）
│   └── stub.rs              ← 旧实现（dead code）
└── window/
    └── mod.rs               ← Lyric 主窗口生命周期（drag / scroll / render / tick / TrayCmd 分发）
```

**观察**：

- 平台抽象扩展为 9 个子 trait，新增 `PlatformWt`、`PlatformShell`（stage-3 新增）。
- `Platform` 总 trait 把 9 个子 trait `+`-bound 在一起，Windows / Stub 各实现一遍。
- services 全部以 `Arc<AppContext>` 注入，与平台解耦。
- `path::resolve_musicfox_data_dir` 直接调 `std::env`，没有走 platform trait —— 因为 `DataDir()` 是 Go 进程的环境语义，不是 lyric 平台能力。**这是合理的设计选择**。

## 2. 平台抽象边界评估

### 2.1 优点

- `Platform` 把 Win32 API 调用集中到 `src/platform/windows/mod.rs`，UI / services / settings 与平台解耦。
- trait 分层清晰（每个子 trait 一类能力），新增能力（如 `PlatformWt`）只需扩展 trait + 实现。
- `StubBackend` 让非 Windows 平台可以 `cargo check` 通过（虽然 stage-3 新增 `set_wt_alpha / is_wt_hidden / install_wt_minimize_hook` 后 Stub 全是 no-op）。

### 2.2 问题

1. **`platform/windows/mod.rs` 单文件 1000+ 行**：Pipe / Tray / WT / Monitor / Shell / WndProc 全在一个文件。可拆分为 `pipe.rs` / `tray.rs` / `wt.rs` / `monitor.rs` / `wndproc.rs`，与 `services/*` 形成上下镜像。
2. **`PlatformPipeClient::send` 返回 `Result<Vec<u8>, String>`**：其他平台 trait 返回 `Result<(), AppError>`。不一致导致 `pipe::reload::notify_reload` 无法复用统一的错误流（参见 P0-2）。
3. **`PlatformShell::reveal_in_file_manager` 与 `PlatformWt::shutdown_wt` 都在 Windows 实现里直接 `std::process::Command::new("explorer.exe").spawn()`**：低权限账号（部分企业环境）`explorer.exe` 不可执行；没有 fallback（ShellExecuteW / Process.Start）。建议 Windows 端改用 `ShellExecuteW(NULL, "open", path, NULL, NULL, SW_SHOWNORMAL)`。
4. **`PlatformTray::init` 签名硬绑 `crate::event_bus::EventBus`**：将来如果想让 tray 模块完全解耦（纯 UI 进程可启动 dummy tray），需要把 event_bus 抽象为 trait。

## 3. Slint ↔ Rust 同步模型

### 3.1 数据流

```
Slint UI (declarative)
   │  properties ↔ callbacks
   ▼
SettingsWindowUi / LyricWindow  (slint 生成的 wrapper)
   │  set_* / get_* / on_*
   ▼
src/settings/mod.rs / src/window/mod.rs
   │  Rc<RefCell<FormState>> / shared state
   ▼
FormState / Runtime  (业务状态)
   │  mark_dirty / flush_tmp_if_due / show_toast
   ▼
disk / IPC pipes
```

### 3.2 不变量

- **属性双向同步**：用 `<=>` 在 Slint 中表示；Rust 端通过 `app.set_*` 写、`app.get_*` 读。`sync_to_ui` 是 Rust → UI 方向唯一入口；`on_changed` 回调是 UI → Rust 方向唯一入口。
- **脏标记隔离**：预览区 / 字体大小等只读字段**不**触发 `form.mark_dirty()`，避免预览调整触发"未保存关闭拦截"（见 P1 评估项 PASS）。
- **toast 时长**：`TOAST_DURATION = Duration::from_secs(3)`，Rust 侧 `form::toast_visible()` 在 timer 100ms 中读到期时间并切 `app.set_show_toast(false)`。

### 3.3 问题（详见 review-finding.md）

1. **线程越界**：`on_changed` / `on_save_clicked` / `on_close_*_clicked` 全部在 Slint 主线程触发；但 `show_tray_popup_menu_for_window` 同步阻塞（P1-2）。
2. **closure 拥有 `form_rc`**：`Rc<RefCell<FormState>>` 在多个 callback 之间共享。`try_borrow_mut` 失败时静默忽略（见 `src/settings/mod.rs:316-318` `if let Ok(mut f) = fr.try_borrow_mut()`）；如果 `sync_from_ui` 还在跑时 timer 也 borrow，会丢弃某些事件 → 表单状态丢失。
3. **`POS_RX` / `POS_WATCHER_RX` 用 `thread_local!` 而不是 `Rc<RefCell<…>>`**：跨 callback + timer 移动易 `borrow_mut` panic（P2-7）。
4. **字体下拉的双源 `index/value`**（P1-6）：Slint 内部维护 `current-index` 与 `current-value` 两份；Rust 同步顺序不一致会引发 ComboBox 显示抖动。
5. **预览区与主文本的 Text 元素复制**：`ui/settings.slint:354-462` 9 个 `Text` 节点（1 主文本 + 8 描边），每个都重复 `text / font-size / font-family / font-weight / font-italic / color` —— 改一处要改 9 处。建议封装 `PreviewText` 子组件。

## 4. 并发模型

### 4.1 线程边界

| 线程 | 启动点 | 持有的状态 |
|---|---|---|
| 主线程（Slint event loop） | `app.run()` | `AppContext`（Arc）/ `Runtime`（Rc）/ `FormState`（Rc）/ `tray_cmd_rx` / `event_bus_rx` |
| UDP listen | `udp::start_listening` | `Arc<AppContext>` |
| Reload pipe server | `pipe::reload::start_server` | `Arc<AppContext>` |
| Pos pipe server | `pipe::pos::start_server` | `Arc<AppContext>` |
| Presence pipe server | `pipe::presence::start_server` | — |
| Tray event | `tray-icon` crate | — |
| Wt init delayed | `wt::init_delayed_check` spawn | `WtConfig` |
| Wt minimize hook | `SetWinEventHook` callback | `OnceLock<Mutex<…>>` 静态 |

### 4.2 共享状态

| 共享状态 | 同步原语 | 风险 |
|---|---|---|
| `Config` | `RwLock` (在 `AppContext::config`) | 锁中毒时 `unwrap_or_else(|p| p.into_inner())` 自恢复；OK |
| `LyricState` | `RwLock` | 同上 |
| `signals` | `AtomicBool` | OK |
| `event_bus` | `crossbeam_channel` | OK |
| `WT_HOOK` / `WT_TARGET_TITLE` | `OnceLock<Mutex<…>>` | 锁泄漏风险（P1-4） |
| `LAST_TOGGLE` / `IS_LAUNCHING` | `OnceLock<Mutex<…>>` | 锁泄漏风险（P1-5） |
| `WND_CTX` | `thread_local!` | 单线程 OK；多线程 `WndProc` 子类化不安全 |

### 4.3 评估

- **优点**：跨线程用 `Arc<AppContext>` + 静态 `OnceLock` 隔离，锁粒度细。
- **问题**：
  1. `app.run()` 主线程同时跑事件循环、drag、scroll、render、tray_cmd 分发；任何阻塞调用（菜单弹出、文件对话框）都会冻屏。
  2. `install_display_change_hook` 用 `SetWindowLongPtrW` 子类化 WndProc，跨线程（来自 win event loop 的窗口消息）调用 `monitor_wnd_proc`，但只持有 `Arc<AppContext>` 中 `signals.display_changed` 一个 AtomicBool，相对安全；但 `thread_local!` 的 `WND_CTX` 在多线程 Win 消息分发时未必一定在主线程（Slint 默认应该只在 UI 线程）。
  3. **多个 `OnceLock<Mutex<…>>` 静态状态**缺乏统一抽象，建议封装成 `LazyLock<WtRuntimeState>` 一次性初始化。

## 5. 资源生命周期

### 5.1 WT Hook 生命周期

```
1. main() 启动
2. window::run()
3. services.wt.init_delayed_check(config.wt.clone())   // spawn 后台线程
4. 1 秒后 → find_wt_window → apply_wt_hosted_style → install_wt_minimize_hook
   → SetWinEventHook 注册全局钩子 → WT_HOOK = Some(hook)
5. 用户点击托盘 → toggle_wt → 可能 launch + 安装钩子（重复安装会被 is_none 守卫）
6. 用户退出 → TrayCmd::Quit → handle_tray_cmd → slint::quit_event_loop()
   → app.run() 返回 → on_exit（当前未调 shutdown_wt，see P1-10）
7. 进程退出 → OS 自动清理
```

**风险**：

- 第 4 步 install 钩子，进程退出**未主动 UnhookWinEvent**（`shutdown_wt` 不被调，见 P1-10）。OS 会自动清理，但下一进程启动时钩子立即可用，存在 ~1s 真空期（P3 需求明确要"1s 后自动托管已存在的 WT"——真空期就是这 1s）。
- `WT_TARGET_TITLE` 全局 Mutex 写入没有时序竞争保护，但只有 install_wt_minimize_hook 写、`wt_minimizestart_proc` 读，且 install 后才有可能 minimize → 实际不会竞争。

### 5.2 UDP socket 生命周期

- `src/main.rs:97-104` 直接 `lyric::udp::bind(receive_port)`，失败 `exit(1)`。
- socket 移动进 `window::run` → `services.udp.start_listening(socket)`。
- `services.udp` 没有显式 shutdown → OS 关闭文件描述符。
- **风险**：进程异常退出时未排空 UDP 接收队列，可能丢最后一行歌词（与 stage-2 同问题，未引入新风险）。

### 5.3 Named pipe 生命周期

- `reload` / `pos` / `presence` 三组 pipe server 都 `thread::spawn` 永久 `loop`，无退出条件。
- 与 lyric 进程同生共死；进程退出时 OS 清理。
- **风险**：阶段 1 进程还在跑时如果 settings 进程打开同名 pipe 失败（ERROR_PIPE_BUSY），这是 stage-2 既有逻辑，未变。

## 6. Slint `Timer::start` 的正确性

`src/settings/mod.rs:520-578` 启动一个 100ms `Repeated` timer 跑防抖 / pos 应用 / toast 同步：

- 每 100ms 一次性 `POS_RX.with(|c| c.borrow_mut().take())` 然后 `try_recv`：能 race 进一次后 channel 被 take 走，下次 timer tick 进 else 分支 `pa.set(true)`。逻辑正确但复杂。
- `POS_WATCHER_RX` 在 timer 中**不 take**，每次都 `try_recv`——意味着 channel 中堆积的事件会逐个消化直到空，OK。
- `flush_tmp_if_due` 在 timer 里跑，无错误回传：`if let Ok(mut f) = fr.try_borrow_mut()`，如果 timer 时机与 callback 重入 borrow 冲突，`try_borrow_mut` 失败 → 静默跳过当次 tick → 500ms 后才补写 tmp。**最坏 100ms 延迟**，可接受。

## 7. 进程模型评估

| 场景 | 期望 | 实际 |
|---|---|---|
| 单实例 lyric | Mutex 持有，独占 lyric 主窗口 | `acquire_main_mutex` 失败 → exit 6 |
| 单实例 settings | Mutex 持有，独占设置窗口 | `acquire_settings_mutex` 失败 → 走 `try_activate_existing` |
| 多 settings 实例 | 第二个激活第一个 | `presence` pipe + `ShowWindow+BringWindowToTop+SetForegroundWindow` |
| lyric ↔ settings IPC | reload / pos 两组 pipe + presence | OK |
| lyric 启动后 WT 已存在 | 1s 后自动托管 | `services.wt.init_delayed_check` |
| lyric 启动后 WT 不存在 | 不动 WT | 默认行为 OK |
| lyric 退出时关 WT | UnhookWinEvent + WM_CLOSE | `on_exit` **未调** shutdown_wt（P1-10） |
| settings 保存触发 reload | 通知 lyric reload | **杀旧拉新**（P0-2） |

## 8. 与 stage-2 的接口兼容

| 接口 | stage-2 | stage-3 | 兼容？ |
|---|---|---|---|
| `notify_reload() -> Result` | 设计 | `()` | 否 |
| 端口变更提示 | 灰色说明文字 | 删 | 否（按 req §3.2.4 要求保留） |
| 字体下拉数据源 | GDI 枚举 | GDI 枚举（impl 路径未变） | 是 |
| 设置 UI 框架 | egui | Slint | 不兼容（按需） |
| `[wt]` 配置节 | 不存在 | 新增 | 后向兼容（缺失走 `WtConfig::default()`） |
| 托盘左键 | OpenSettings | ToggleWt | 不兼容 |
| 托盘右键菜单 | 含「go-musicfox」项 | 不含 | 不兼容（按 P0-P3-bug.md BUG-03 要求移除） |
| `event_bus::AppEvent::ConfigReloaded` | 存在 | 存在 | 是 |
| `event_bus::AppEvent::RequestRepaint` | 存在 | 存在 | 是 |

## 9. 改进优先级建议

1. **删死代码**（P1-1）：tray/windows_tray.rs、tray/stub.rs、services/tray.rs。
2. **恢复 `notify_reload` IPC 语义**（P0-2）：让 req §2.3 的两个 toast 路径成为可达用户故事。
3. **加"重载歌词"按钮**（P0-1）。
4. **修托盘菜单底部边界**（P0-3）：补 `MONITORINFO + GetSystemMetrics(SM_CYMENU)` 上推。
5. **修 ScrollView**（P0-5）：按 P0-P3-bug.md 要求改为自然流式。
6. **修卡片顺序**（P0-4）：把 go-musicfox 卡片挪到「系统与网络」之后。
7. **`on_exit` 调 `shutdown_wt`**（P1-10）：进程退出时清理钩子并关 WT。
8. **WT minimize hook 加进程名校验**（P1-4）：避免误命中。
9. **补测试**：resolve_musicfox_data_dir 4 级 fallback、notify_reload 的 Result 行为。
10. **拆分 `platform/windows/mod.rs`**（P2-1）：按子能力拆为多文件。