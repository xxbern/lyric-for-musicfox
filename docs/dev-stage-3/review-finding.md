# Stage-3 Code Review 发现清单

> 评级口径：
> - 🔴 **P0 必修**：违反 req.md 验收点 / 引入功能回退 / 用户故事断裂
> - 🟠 **P1 重要**：dead code / 资源泄漏 / 跨线程不安全 / 文档漂移
> - 🟡 **P2 建议**：可读性 / 一致性 / 微小不一致

## 1. 🔴 P0-1：设置 UI 没有"重载歌词"按钮（违反 req §2）

**位置**：`ui/settings.slint`、回调缺失。

**证据**：

1. `ui/settings.slint` 底部操作栏只有 `💾 保存`（primary）与 `📁 打开配置目录`（secondary），**没有**"重载歌词"按钮。
2. `ui/settings.slint:113-123` callback 列表里也没有 `reload-clicked` / `reload-歌词` 类回调。
3. `src/settings/mod.rs` 整个文件 `grep -n "reload"` 只命中 `restart_lyric_app` 相关行，无 UI 绑定。

**影响**：

- req §2.2/§2.3 验收的所有场景在当前 UI 中**没有入口**，F10（"重载歌词独立反馈"）无法走通任何用户故事。
- 用户只能依赖"保存并重启"间接达到重载效果，但 `restart_lyric_app()` 杀旧拉新的方式会让所有"上一次播放"上下文中断。

**建议**：在 `ui/settings.slint` 底部操作栏的 `📁 打开配置目录` 之后追加 `🔄 重载歌词` 按钮，并在 `src/settings/mod.rs` 加 `on_reload_clicked` 回调中走"写 reload 管道 → Ok toast / Err toast"路径（详见下条）。

---

## 2. 🔴 P0-2：`notify_reload()` 返回 unit，违背 req §2.2 IPC 设计

**位置**：`src/pipe/mod.rs:18-22`、`src/platform/windows/mod.rs:978-997`、`src/settings/form.rs:154-156`。

**证据**：

```rust
// src/pipe/mod.rs:18
pub fn notify_reload() {
    #[cfg(windows)]
    crate::platform::windows::restart_lyric_app();
    ...
}
```

```rust
// src/settings/form.rs:152-156
self.dirty = false;
self.last_change = None;

// 保存后直接重启歌词主进程
std::thread::spawn(move || {
    crate::pipe::reload::notify_reload();
});
```

实际行为：保存后**杀旧主进程 → 250ms 后拉新主进程**（`restart_lyric_app`），不走 `reload-{session_id}` 命名管道。

**对比设计**：

- req §2.2 第 1 条："设置界面「重载歌词」按钮按下时，**复用**主进程已有的「重载」语义：调用 reload 管道通知主进程重载"。
- req §2.2 第 5 条："「保存并应用」按钮的行为不变：原子写 `config.toml` + 通知主进程重载 + 成功 toast「配置已保存并已实时生效」"。
- dev.md §P0 Step 2 给出标准实现：

  ```rust
  pub fn notify_reload() -> Result<(), crate::error::AppError> { ... }
  ```

**实际与设计差异**：

1. 返回类型从 `Result<(), AppError>` 退化为 `()`。
2. 通知实现从「写命名管道」退化为「杀进程 + 重启」。
3. 设置 UI 的 toast 只能是"配置已保存"一种结果（src/settings/mod.rs:341），没有差异化路径。

**影响**：

- req §2.3 验收表的两个场景（在线 / 离线）都无法演示。
- 杀旧拉新会让正在播放的 go-musicfox（虽然是外部进程，但用户的预期是"无缝重载"）如果通过 lyric-for-musicfox 触发的 WT 会话被牵连（待确认 `restart_lyric_app` 路径是否关 WT）。

**验证 `restart_lyric_app` 与 WT 的关系**：`src/window/mod.rs:419` `TrayCmd::Quit` 路径调用 `services.wt.shutdown(&ctx.get_config().wt.title)` —— 正常退出是关 WT 的；但 `restart_lyric_app` 直接发 `WM_CLOSE` 给 lyric 主窗口，**WT 不会被主动关闭**（go-musicfox 进程继续跑）。这意味着用户每次保存配置都会留下一个孤儿 WT 进程。

**建议**：

1. 恢复 `notify_reload() -> Result<(), AppError>` 签名，向 `reload-{session_id}` 管道写 `IpcMessage::ReloadConfig`。
2. `form::flush_and_save_now` 改为 `notify_reload()` → Ok / Err 决定 toast（`Ok` → "配置已保存并已实时生效"；`Err` → "主进程未运行，无法重载"）。
3. 若仍要保留"杀旧拉新"作为兜底，至少只在 `Err(AppPath)` 时才走重启路径。

---

## 3. 🔴 P0-3：托盘菜单底部仍可能溢出屏幕

**位置**：`src/platform/windows/mod.rs:240-245` `TrackPopupMenuEx` 调用。

**证据**：

```rust
let cmd_id = TrackPopupMenuEx(
    hmenu,
    (TPM_TOPALIGN | TPM_LEFTALIGN | TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
    pt.x,
    pt.y,                 // ← 直接用 pt.y
    target_hwnd,
    None,
);
```

`pt` 来自 `GetCursorPos`（`src/platform/windows/mod.rs:204-206`），未与 `GetMonitorInfo(MONITORINFO.rcWork)` 计算菜单高度后做向上位移。

**对比 req**：

- req §1.2 要求 1.4 验收："鼠标在屏幕任意 Y 位置按下右键，菜单顶部边界 ≤ 鼠标 Y，且菜单底部边界 ≤ 屏幕物理像素高度"。

**当前实现只能满足前者**。

**影响**：1080p 屏幕右下角托盘右键，若菜单总高度 > 鼠标 Y 到屏幕底部的距离（例如 3 个 32px 菜单项 ≈ 120px 还能装下，但加 8px 分隔条和滚动描述就够呛；尤其当 OS DPI 缩放 / 用户加 4K 分辨率时极易溢出），菜单底部会被屏幕边缘吃掉。BUG-01 的根因分析（`P0-P3-bug.md` §2）只切对了 `with_menu` 这一半。

**建议**：

```rust
// 伪代码
let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
let mut mi = MONITORINFO { cbSize: ..., ..Default::default() };
GetMonitorInfoW(monitor, &mut mi);
// 估算菜单高度：菜单项数 * GetSystemMetrics(SM_CYMENU) + 边框
let menu_height = items_count * GetSystemMetrics(SM_CYMENU) + 4;
let y = if pt.y + menu_height > mi.rcWork.bottom {
    mi.rcWork.bottom - menu_height
} else {
    pt.y
};
let cmd_id = TrackPopupMenuEx(
    hmenu,
    (TPM_TOPALIGN | TPM_LEFTALIGN | TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
    pt.x, y, target_hwnd, None,
);
```

---

## 4. 🔴 P0-4：设置卡片顺序违反 req §6.2.1

**位置**：`ui/settings.slint`。

**证据**：

| 顺序 | 卡片 | 行号 |
|---|---|---|
| 1 | 🪟 窗口与定位 | 165 |
| 2 | 🎨 歌词与样式 | 209 |
| 3 | 🎵 **go-musicfox** | 459 |
| 4 | ⚙️ 系统与网络 | 528 |

req §6.2.1 写："放置在「系统与网络」卡片**之后**"。

期望：1 → 2 → 4 → 3（窗口 → 样式 → 系统 → go-musicfox）。

**影响**：与需求文档不一致；用户预期"系统设置 → 第三方应用设置"的递进顺序被颠倒。

**建议**：调换 458 / 528 两个 `Rectangle` 块顺序即可。

---

## 5. 🔴 P0-5：`ui/settings.slint` 仍使用 `ScrollView`（违反 `P0-P3-bug.md` BUG-04 修复 #2）

**位置**：`ui/settings.slint:130`。

**证据**：

```slint
ScrollView {
    vertical-stretch: 1;
    VerticalLayout { ... }
}
```

而 `P0-P3-bug.md` §3 第 2 项明确写：

> **消除滚动背景**：调整窗口整体高度（如 680px）与各卡片间距（10px），移除外层 `ScrollView`，改为自然流式布局，保证 100% 无滚动条。

**对比当前**：

- `preferred-height: 520px`，`min-height: 360px`。
- 实际内容：4 卡片 + 标题 + 底部操作栏 + 间距，合计估算高度 ≥ 580px。

**影响**：在 520px `preferred-height` 下窗口初始尺寸下仍会出滚动条；`min-height: 360px` 进一步压缩时滚动条更明显。

**建议**：按 bug 报告要求改为自然流式 `VerticalLayout`，并把 `preferred-height` 调到 `680px` / `min-height` 调到 `600px`，同时压紧各卡 `padding: 10px` 与 `spacing: 6px`（已是 6px，OK）。

---

## 6. 🟠 P1-1：三份 `TrayCmd` 定义共存 + 死代码模块

**位置**：

- `src/tray/mod.rs:5-11` ← 唯一被引用的版本（在 `crate::tray::init_tray` / `TrayCmd` 导出）
- `src/tray/windows_tray.rs:11-17` ← **无 `mod` 引用**
- `src/tray/stub.rs:5-11` ← **无 `mod` 引用**
- `src/services/tray.rs` ← `TrayService::init_tray` 也**无调用点**（`grep -n "services\.tray\." src/` 0 命中；`ServiceHandles.tray` 字段也未被 `mod.rs` 之外任何地方 `services.tray.foo()` 调用）

**证据**：

```bash
$ grep -rn "tray::windows_tray" src/    # 0 命中
$ grep -rn "mod stub" src/tray/mod.rs    # 0 命中
$ grep -rn "services\.tray" src/          # 0 命中（除了 services/mod.rs 自身）
$ grep -rn "windows_tray" Cargo.toml src/lib.rs src/main.rs  # 0 命中
```

**对比 stage-2 / stage-1 的模块清理**：`src/bin/mock_udp_sender.rs` 与 `tests/mock_udp_sender.rs` 是新拆分的合理形态，但 `src/tray/windows_tray.rs` 是 stage-3 重构**遗漏的旧版**实现（保留了 `with_menu` + 内置菜单的旧逻辑），与 `src/platform/windows/mod.rs` 的新实现是平行实现。

**影响**：

- 二进制体积：未用代码被静态链接（debug build ~+30KB；release LTO 通常会清掉，但破坏职责清晰）。
- 阅读困惑：未来维护者看到 `tray/windows_tray.rs` 的菜单构造代码会怀疑到底走哪条路径；尤其 `windows_tray.rs` 与 `platform/windows/mod.rs::show_tray_popup_menu_for_window` 实现完全不同的菜单生成方式。

**建议**：

1. 删除 `src/tray/windows_tray.rs`（旧实现，203 行）。
2. 删除 `src/tray/stub.rs`（Stub 通过 `platform/stub/mod.rs::PlatformTray` 已覆盖）。
3. 删除 `src/services/tray.rs`（`TrayService` 与 `crate::tray` 全重合，未被使用）。
4. `src/services/mod.rs::ServiceHandles` 删去 `pub tray` 字段。
5. 同步 `src/lib.rs`（如有 `pub mod tray` 之外再 `pub mod services::tray`，保持单一定义）。

---

## 7. 🟠 P1-2：`ShowContextMenu` 在 Slint tick 中同步阻塞

**位置**：`src/window/mod.rs:404-413`。

**证据**：

```rust
crate::tray::TrayCmd::ShowContextMenu => {
    #[cfg(windows)]
    {
        let hwnd = r.hwnd.unwrap_or(windows::Win32::Foundation::HWND(std::ptr::null_mut()));
        let (tx, rx) = crossbeam_channel::unbounded();
        crate::platform::windows::show_tray_popup_menu_for_window(hwnd, &tx);
        while let Ok(sub_cmd) = rx.try_recv() {
            handle_tray_cmd(sub_cmd, ctx, services, r);
        }
    }
}
```

`TrackPopupMenuEx` 是**模态阻塞** Win32 API（直到用户点选 / 关闭菜单才返回），而这里在 Slint 16ms tick 回调里调用它，意味着：

1. 整个渲染循环被冻结（菜单弹出期间，歌词窗口不刷新、不响应拖拽、不响应 UDP 歌词包）。
2. 如果用户在菜单弹出期间点选「退出」，`rx.try_recv()` 会拿到 `TrayCmd::Quit` → `slint::quit_event_loop()`，但因为还在 tick 中，循环本身是阻塞的，菜单关闭后才回到 tick；菜单期间若有 timeout（例如 tray 卡死）整个 GUI 冻死。

**影响**：菜单弹出期间 100~500ms 内 GUI 完全冻结（用户肉眼可见）；高频右键有概率触发可感知的卡顿。

**建议**：

- 把 `TrackPopupMenuEx` 调度到独立 OS 线程，事件通过 `event_bus` 发回主线程消费。
- 或者：让菜单弹出前先 `event_bus.emit(RequestRepaint)` 刷新一帧，然后让 Slint 的事件循环在用户点选菜单后再唤醒（但 Slint 没有"消息泵 yield"接口，需要走第二种方案）。

---

## 8. 🟠 P1-3：`install_display_change_hook` 永久接管 lyric 主窗口 `WndProc`

**位置**：`src/platform/windows/mod.rs:683-697`。

**证据**：

```rust
fn install_display_change_hook(&self, hwnd: super::WindowHandle, ctx: Arc<AppContext>) {
    ...
    unsafe {
        let prev = SetWindowLongPtrW(
            hwnd,
            GWL_WNDPROC,
            monitor_wnd_proc as *const () as isize,
        );
        if prev != 0 {
            let prev_wndproc: WNDPROC = std::mem::transmute(prev);
            set_prev_wnd_proc(prev_wndproc);
        }
    }
}
```

1. `set_prev_wnd_proc` 仅存一份 `prev_wnd_proc`，但只调一次。
2. Slint 自身**已经**接管了 lyric 窗口的 `WndProc`（Skia/Femtovg 后端都需要）。这次再覆盖一次，等于把 Slint 的 WndProc 替换成我们的 `monitor_wnd_proc`，再由我们手动转发给 Slint 的 prev。

**风险**：

- 如果 Slint 自身后续还要换 WndProc（例如多后端切换 / 重新初始化），链路会断。
- 仅在 lyric 主进程入口调一次，没有卸载逻辑（歌词主窗口退出时 WndProc 随之销毁，OK；但中途更换 WndProc 的情况未覆盖）。
- `monitor_wnd_proc` 处理 `WM_DISPLAYCHANGE (0x007E)` 后透传 `prev`，但 `prev` 拿到的地址可能在后续 Slint 重新初始化后失效。

**建议**：

- 改用 Slint 提供的 `window.on_scale_factor_changed` / `set_position` 替代 WndProc 子类化；
- 或确认 Slint 后端是否提供 `raw-window-handle-06` 之外的事件钩子（`PlatformWindow::request_redraw` 等），改走 Slint 原生路径。

---

## 9. 🟠 P1-4：`MINIMIZE_START` 全局钩子的误命中风险

**位置**：`src/platform/windows/mod.rs:786-822`。

**证据**：

- `SetWinEventHook(EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZESTART, NULL, proc, 0, 0, WINEVENT_OUTOFCONTEXT)` —— `pid=0, thread=0` 表示监听**系统所有进程**的最小化事件。
- 匹配规则是窗口标题 `contains(target_title)`（`MusicFoxTerminal`）。

**风险**：

1. 如果用户在自己项目里把某个窗口标题也命名为 `MusicFoxTerminal`（或子串），最小化时会被劫持。
2. Windows Terminal 本身可能在 pane 切换 / tab 切换时弹出临时子窗口，这些窗口可能携带主窗口文本片段，**理论上**会被误命中。
3. 多 WT 实例（用户开两个终端）时，钩子会被触发多次，每次都生成一个线程 `thread::spawn`，存在资源泄漏。

**建议**：

- 在 `wt_minimizestart_proc` 中增加**进程名校验**：`GetWindowThreadProcessId(hwnd, Some(&mut pid))` + `ProcessIdToSessionId` / `GetModuleFileNameW` 校验属于 `WindowsTerminal.exe`。
- 多实例场景下，使用一个 `HashSet<HWND>` 跟踪"已托管"的窗口，仅处理集合内的最小化事件。

---

## 10. 🟠 P1-5：`WtService::toggle` 反复重读磁盘 + 全局静态状态的锁竞争

**位置**：`src/services/wt.rs:48-96`。

**证据**：

- `toggle` 每次都调 `crate::services::config::ConfigService::load_or_default()` 读磁盘。
- 同时访问 4 个 `OnceLock<Mutex<…>>` 静态状态：`LAST_TOGGLE` / `IS_LAUNCHING` / `WT_HOOK` / `WT_TARGET_TITLE`。
- 锁顺序未统一：`LAST_TOGGLE` → `IS_LAUNCHING` → `current_cfg` → 写回 `IS_LAUNCHING`。两次调 toggle 时序竞争窗口 400ms。

**风险**：

- 高频点击托盘时（连续 5 次在 400ms 内），每次都读 `config.toml`（~5ms），并发 16ms tick 中可能拖慢 lyric 渲染。
- 锁泄漏：`IS_LAUNCHING = true` 后如果 launch 异常，spawn 的清理路径只在 `Err(e)` 路径释放；正常路径下等 1s 后由 `std::thread::spawn` 块尾部释放。**如果 OS 中途异常杀线程**，`IS_LAUNCHING = true` 会永久卡死 toggle。

**建议**：

- 缓存配置（不每次读盘）；只在 `event_bus::AppEvent::ConfigReloaded` 时刷新。
- 给 `IS_LAUNCHING` 设置**超时解锁**（例如 5s 后强制 false）；或用 RAII 守卫。

---

## 11. 🟠 P1-6：UI 同步的 `font_current_value` 与 `font_current_index` 双源不一致

**位置**：`src/settings/mod.rs:178-185`、`src/settings/mod.rs:181` 与 `ui/settings.slint:227`。

**证据**：

- Slint `ComboBox` 同时绑定 `current-index` 与 `current-value` 两个属性。
- Rust `sync_to_ui` 同时设置这两个。
- 用户在 ComboBox 下拉点选 → `on_font_selected(idx)` 回调 → `set_font_current_value(name)` + `set_font_current_index(idx)`。
- 但 `sync_from_ui` 中：`let family = app.get_font_current_value().to_string()` —— 实际依赖 `current_value`，而 `current_index` 仅在用户**通过下拉点选**时更新。如果外部修改 `current_value`（如 `font_color` 改变），`current_index` 可能 stale → ComboBox 显示不匹配。
- 同理 `font_selected` 回调里先 `set_font_current_index(idx)` 再 `set_font_current_value(name)`，顺序正确但 `sync_to_ui` 里是先 `set_font_current_value` 再 `set_font_current_index`（顺序不一致可能触发 Slint 渲染 jitter）。

**风险**：

- 用户打开设置后字体下拉显示的不是当前选中项。
- font-preview 的 `font-family` 与实际选中值不一致（`root.font-current-value` 跟随 value，UI 显示跟随 index）。

**建议**：

- 单源：在 Rust 侧维护 `Vec<String>` 索引表，下拉事件只发 `index`，Rust 查表回填 `font_current_value`。
- 或：每次 `sync_to_ui` 先 `set_font_model` 再 `set_font_current_value` 再 `set_font_current_index`。

---

## 12. 🟠 P1-7：`validate_musicfox_path` 在 `cfg!(windows)` 之外的逻辑很奇怪

**位置**：`src/settings/validate.rs:65-79`。

**证据**：

```rust
#[cfg(not(windows))]
{
    let p = std::path::Path::new(s);
    if !s.starts_with("C:") && !s.starts_with("c:") && !p.exists() {
        return Err("go-musicfox 可执行文件不存在".into());
    }
}
```

非 Windows 下当路径以 `C:` 开头时**直接通过校验**，无论文件是否存在。

**影响**：

- 这是一段疑似 stage-1/stage-2 留下的兼容代码；本项目目标是 Windows，跨平台分支不应该有"以 C: 开头就放过"这种逻辑。
- 单元测试 `tests/validate_test.rs::musicfox_path_validation` 在 Linux 上跑会跳过这个分支，存在测试盲区。

**建议**：删除非 Windows 分支，或改为 `#[cfg(not(windows))] { p.exists() }` 简单校验；并在测试中增加覆盖。

---

## 13. 🟠 P1-8：`ui/settings.slint` 错误提示位置固定于底部，不能指明字段

**位置**：`ui/settings.slint:691-700`、`src/settings/mod.rs:351`。

**证据**：

- `app.set_save_error(...)` 是底部操作栏的统一红字，没有 per-field 红字。
- req §6.2.3 要求："字段下方提供校验错误红字（沿用现有 `field_errors` 机制）"。
- 当前没有 `field_errors` Slint 属性；也没有逐字段红字渲染。

**影响**：用户保存失败时只能看到 "musicfox_path: go-musicfox 可执行文件不存在" 一行文字，无法定位是哪个字段出错。

**建议**：在 `ui/settings.slint` 各字段后追加 `Text { color: #ba1a1a; font-size: 11px; text: ... }`，并新增 `in-out property <string> musicfox-path-error` 等，由 Rust 侧 `validate_all` 拆字段填入。

---

## 14. 🟠 P1-9：`lyric.slint` 与 `settings.slint` 描边宽度的单位混用

**位置**：`ui/lyric.slint:11, 169`、`ui/settings.slint:351`。

**证据**：

- `lyric.slint:11` `font-size-px` 是物理像素（`out property <length> font-size-px`）。
- `lyric.slint:169` `outline-w: outline-width-px * 1px` → 物理像素。
- `settings.slint:351` `outline-w: root.outline-width-text.to-float() * 1px` ← 这里 `outline-width-text` 来自 `form.draft.lyric_style.font_outline_width`（u32，单位 DIP）。
- 但 `outline-width-px` 在 Rust 侧是 `cfg.lyric_style.font_outline_width.max(0) as i32`（`src/window/mod.rs:80`），单位是 DIP。

**结论**：lyric 描边宽度是 DIP，没有 DPI 缩放补偿；settings 描边宽度也是 DIP（同样无补偿）；预览与实窗**单位一致**但都未做 DPI 缩放。在 100% DPI 下没问题；在 125% / 150% DPI 下描边视觉宽度比字号相对变细。

**建议**：

- 统一改用 `1px` 物理像素（或统一为 `1dip`），并在 `apply_style_properties` 中乘 `scale_factor`：

  ```rust
  app.set_outline_width_px((cfg.lyric_style.font_outline_width.max(0) as f32 * app.window().scale_factor()).round() as i32);
  ```

---

## 15. 🟠 P1-10：歌词主进程退出未调用 `shutdown_wt`

**位置**：`src/window/mod.rs:439-451`。

**证据**：

```rust
fn on_exit(rt: &Shared<Runtime>, ctx: &Arc<AppContext>, _services: &ServiceHandles) {
    ...
}
```

`_services` 被丢弃。`services.wt.shutdown(...)` 未调用。

**对比 req**：req §5.2.8 明确"退出主进程时清理钩子并关闭 WT 窗口"。

**影响**：

- 主进程通过 `TrayCmd::Quit` 退出时，`WT_HOOK` 不会被卸载 → 进程退出后 OS 自动清理（OK），但下次启动 lyric 时钩子装回去之前 1s 窗口里没有钩子 → 已存在的 WT 最小化会被最小化（不会透明隐藏）。
- WT 窗口不会主动关闭 → go-musicfox 继续在后台跑。
- 用户体验："退出 lyric 后 WT 还在"，但歌词没了。

**建议**：

```rust
fn on_exit(rt: &Shared<Runtime>, ctx: &Arc<AppContext>, services: &ServiceHandles) {
    services.wt.shutdown(&ctx.get_config().wt.title);
    ...
}
```

---

## 16. 🟠 P1-11：`tests/settings_test.rs::toast_lifecycle_visible_and_expires` 只断言字符串字面量

**位置**：`tests/settings_test.rs:181-188`。

**证据**：

```rust
#[test]
fn toast_lifecycle_visible_and_expires() {
    ...
    form.show_toast("已发送重载通知");
    assert_eq!(form.toast_visible(), Some("已发送重载通知"));
}
```

**影响**：没有覆盖 req §2 的两个关键场景：

- 设置进程点击"重载歌词"，主进程运行 → toast "已发送重载通知"。
- 设置进程点击"重载歌词"，主进程未运行 → toast "主进程未运行..."。

但当前 UI 没有"重载歌词"按钮 → 即便加测试也无法触达。修 P0-1 后再补端到端测试。

---

## 17. 🟠 P1-12：`update_preview_brushes` 与 `sync_to_ui` 都设同一组 brush，存在重复

**位置**：`src/settings/mod.rs:139-143` 与 `src/settings/mod.rs:188`。

**证据**：`update_preview_brushes(app)` 在 `sync_to_ui` 末尾与 `on_changed` 回调里都被调用，每次都会做 `parse_color_or_default` + `parse_color_or_default`。

**影响**：20 行重复代码；修改预览颜色解析规则需要改两处。

**建议**：提取为辅助函数 `sync_preview_brushes(app: &SettingsWindowUi)`，统一调用。

---

## 18. 🟡 P2 列表

| # | 位置 | 内容 |
|---|---|---|
| P2-1 | `src/platform/windows/mod.rs:39-40` | `use windows::Win32::UI::WindowsAndMessaging` 引入列表 80+ 行；建议按子模块分组（Pipe / Window / Shell / Hook）。 |
| P2-2 | `src/platform/windows/mod.rs:204-206` | `GetCursorPos` 失败时 `pt` 是 `POINT::default()` = (0,0)，会把菜单弹到屏幕左上角；建议失败时跳过弹菜单。 |
| P2-3 | `src/platform/windows/mod.rs:819-845` | `find_window_by_title_contains` 既用 `FindWindowW` 又用 `EnumWindows` 重复匹配；建议直接走 `EnumWindows`，并去除 `FindWindowW` 失败后回退路径。 |
| P2-4 | `src/settings/mod.rs:159-185` | `sync_to_ui` 函数 30+ 行属性赋值；可考虑用 `FormState → UiProps` 反射赋值，或拆为多个 `sync_*_to_ui`。 |
| P2-5 | `src/services/wt.rs:53-56` | `load_or_default` 失败时回退 `self.ctx.get_config()` —— 但 `ctx` 是 `Arc<AppContext>`，主进程启动时配置可能尚未 load；建议至少打 warn 日志。 |
| P2-6 | `ui/settings.slint:8-21` | `ColorPill` 组件用 `TouchArea` + `clicked` 而不是 `Button`；触屏用户友好但键盘导航缺失（Tab 不到）。 |
| P2-7 | `src/settings/mod.rs:594-613` | `POS_RX` / `POS_WATCHER_RX` 用 `thread_local!` 保存 `RefCell<Option<Receiver<…>>>`，跨回调 + 跨 timer 移动，容易引起 `borrow_mut` panic；建议改为 `Rc<RefCell<…>>` 通过 `app.set_user_data` 存到 Slint 组件实例上。 |
| P2-8 | `src/path.rs:55-62` | `musicfox_data_dir_display()` 没有处理 `Option::None` 时返回的"当前：无法定位" 的本地化字符串；应统一 I18N 策略或保持硬编码至少要写在 `path.rs` 顶部常量。 |
| P2-9 | `tests/settings_test.rs::bootstrap_prefers_valid_tmp_over_config` | 没有断言 `dirty = true` 时的恢复路径在 UI 层 (`show_toast("已恢复未保存的修改")`)；dev.md §8.2 F03 提到"应弹出红字提示"，目前缺失。 |
| P2-10 | `src/main.rs:43-46` | panic hook 用 `eprintln!` 双输出；`windows_subsystem = "windows"` 时双击启动看不到 stderr；应只写文件。 |
| P2-11 | `src/window/mod.rs:120` | `app.window().set_size(...)` 用 `PhysicalSize`，但 `services.wt.init_delayed_check(...)` 用的是逻辑单位 `config.wt.clone()`，两边单位要在 DPI 缩放下重新核算一次。 |
| P2-12 | `src/services/wt.rs:30-37` | `LAST_TOGGLE` 静态初始化为 `Instant::now() - Duration::from_secs(10)` 是 hack；建议直接用 `OnceLock<Instant>`，首次访问时 `Instant::now()` 初始化。 |
| P2-13 | `src/services/wt.rs:97-108` | `std::thread::spawn` 内部的 `if let Some(hwnd) = ... find_wt_window(&title)` —— title 是 String 移动进闭包，spawn 闭包外还有 `*launching = false` 在 `else` 路径释放；逻辑正确但**缩进嵌套深**，建议拆 `try_apply_wt_style(title: String)` 辅助函数。 |
| P2-14 | `src/platform/windows/mod.rs:374-378` | `ReadFile` / `WriteFile` 都用 `BufReader::read_line`，没有超时控制；如果主进程卡死，pos 管道会无限挂起（虽然有 `ManuallyDrop`，但 Reader 持有期间 `DisconnectNamedPipe` 不会被调）。 |
| P2-15 | `src/window/mod.rs:419` | `services.wt.shutdown(&ctx.get_config().wt.title)` —— 但 `on_exit` 不调用（见 P1-10），且 `_services` 被丢弃，是潜在的 undefined。 |

## 19. 文档漂移（dev.md ↔ 实际实现）

| 项 | dev.md 描述 | 实际实现 |
|---|---|---|
| 端口说明行 | dev.md §P1 Step 3: "字段后附说明文本：`Text { text: '端口变更需重启主进程生效；此处只读'... }`" | 不存在 |
| `notify_reload` 签名 | dev.md §P0 Step 2: `Result<(), AppError>` | `()` |
| ScrollView | dev.md §P1 Step 2: "各区块卡片分明" + P0-P3-bug.md 要求移除 | 仍存在 |
| Input 高度 | dev.md §P1 Step 3: "紧凑工整"；P0-P3-bug.md 要求 32px | 30px |
| `[wt]` 配置 | dev.md §P3 Step 1 默认值 `C:\Users\xx\app\musicfox\musicfox.exe` | 同 |
| 字体列表 | dev.md §P0 Step 4: "Ensure `crate::platform::current().send` 在管道不存在时返回具体的 `AppError::PipeConnectionFailed` 或 `AppError::Io`" | `PlatformPipeClient::send` 返回 `Result<Vec<u8>, String>`，不是 `AppError` |
| `apply_locked_style` | dev.md §P3 Step 3: "增加 `WS_EX_TOOLWINDOW`" + "移除 `WS_CAPTION`" 描述的是 WT | lyric 窗口用 `apply_locked_style` 不动 caption（符合实际），但 dev.md 在 §3 描述里把这两块搅在一起 |
| dev.md §4 平台抽象 | `calculate_safe_menu_pos` 在表中 | 实际并未提供该函数；`show_tray_popup_menu_for_window` 内联做 `pt.y` 推导，未做边界检测 |
| F11 字体枚举 | dev.md §6: "本机 GDI/系统已安装的字体族名" | `services/font.rs::system_families()` 实现，无 GDI 字面描述 |
| go-musicfox 卡片位置 | dev.md §P4 Step 2: "在「系统与网络」卡片之后追加" | 在「系统与网络」之前 |
| 预览背景高度 | dev.md §P2 Step 1: "高度固定 80px" | `60px` |
| 预览字号单位 | dev.md §P2 Step 1: "字号：绑定 `root.font-size-pt * 1pt`" | lyric 侧是 `font-size-px`（pixel）；settings 预览是 `font-size-pt * 1pt`，两边未统一 |

---

## 20. 安全 / 一致性

| 项 | 内容 |
|---|---|
| `rfd = "0.15"` | 与 Slint 1.x 兼容；但 `rfd` 默认行为是同步阻塞，与 Slint tick 共用线程（参见 P1-2） |
| `tray-icon = "0.19"` | 与 Windows 0.58 API 一致；运行时已不需要 with_menu |
| `windows = "0.58"` | feature 列表完整；`Win32_UI_Accessibility` 已启用 |
| `slint` 默认 `material` | `std::env::set_var("SLINT_STYLE", "material")` 在 `main.rs:29` 和 `settings/mod.rs:236` 双重设置，建议统一放到 `lib.rs::init()` |
| 编译警告 | 未跑 `cargo build`；review 阶段未涉及 |

## 21. 测试覆盖观察

| 模块 | 测试文件 | 覆盖度观察 |
|---|---|---|
| `config` | `tests/config_test.rs` | 充分（默认值 / 缺失字段 / 空串 ↔ None / 非法值 / `[wt]` 节缺失回退） |
| `settings::form` | `tests/settings_test.rs` | 充分（bootstrap / 防抖 / 原子提交 / parse modal / discard） |
| `settings::validate` | `tests/validate_test.rs` | 中（边界 + 字体校验在非 Windows 上跳过） |
| `path` | **无** | 缺 `resolve_musicfox_data_dir` 单元测试（req §6.3 4 级 fallback 全场景） |
| `services::wt` | **无** | toggle / launch / shutdown 全链路无测试 |
| `platform::windows::show_tray_popup_menu_for_window` | **无** | 菜单弹出 / Y 边界 / 屏幕底部溢出等无测试 |
| `minimize_start_proc` | **无** | 钩子回调逻辑无模拟测试 |
| `restart_lyric_app` | **无** | 杀旧拉新 + WT 关系无端到端验证 |
| `form::flush_and_save_now` → toast 联动 | **弱**（仅 `toast_lifecycle_visible_and_expires` 测字面量） | req §2 差异化 toast 无验证 |

**建议**：至少补 5 个新测试：

1. `resolve_musicfox_data_dir`：4 种环境变量优先级。
2. `validate_all` 对 `wt.musicfox_path` 的空 / 不存在 / 合法三情况。
3. `parse_pos_response` 接受 `POS ` 前缀、空字段、半填充（`100,EMPTY`）。
4. `notify_reload_returns_err_on_no_main`：req §2.2 / F10。
5. `settings::form::flush_and_save_now` 在不同脏状态下的 notify_reload 调用次数。