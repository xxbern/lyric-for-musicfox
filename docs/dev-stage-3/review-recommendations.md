# Stage-3 修复建议（按优先级）

> 排序口径：先修会破坏用户故事的 P0，再修会让下一次维护者迷路的死代码 / 资源泄漏，最后修一致性 / 体验。
> 每条都附：影响、改动点、验证方式、预估工作量。
> **本文件不是修复实施方案**——只列方向。

## 批次 1：必修（P0，阻塞 F10 / 验收 §1.4 / 视觉一致性）

### 1.1 恢复 `notify_reload()` 的 IPC 语义

**影响**：阻塞 req §2.3 全部验收；导致 P0-P3-bug.md 提到的"已发送重载通知 / 主进程未运行"差异化 toast 路径不存在。

**改动**：

- `src/pipe/mod.rs::notify_reload` 签名改回 `pub fn notify_reload() -> Result<(), AppError>`，写 `reload-{session_id}` pipe 的 `IpcMessage::ReloadConfig`。
- `src/settings/form.rs::flush_and_save_now` 末尾的 `crate::pipe::reload::notify_reload()` 同步等待结果：
  - `Ok(())` → `show_toast("配置已保存并已实时生效")`
  - `Err(AppError::PipeConnectionFailed)` 或 `Err(_)` → `show_toast("主进程未运行，重启 lyric-for-musicfox 后生效")`
- 删除 `crate::platform::windows::restart_lyric_app` 的调用，或仅保留为 fallback（在 IPC 完全不可用时仍可救场）。

**验证**：

- 单元：`tests/pipe_test.rs` 加 `notify_reload_returns_err_when_no_main_process_running`。
- 集成：手动起 settings → 起 lyric → 点保存 → 验证 lyric 进程**不退出**，仅重载配置；toast 文本为"配置已保存并已实时生效"。
- 反例：手动起 settings → 不起 lyric → 点保存 → toast 文本为"主进程未运行..."。

**预估工作量**：4~6 小时（含测试）。

---

### 1.2 设置 UI 加"重载歌词"按钮

**影响**：F10 用户故事入口。

**改动**：

- `ui/settings.slint` 底部操作栏追加 `🔄 重载歌词` 按钮（次级样式，`primary: false`）。
- `src/settings/mod.rs` 新增 `on_reload_clicked` 回调：

  ```rust
  app.on_reload_clicked(move || {
      if let (Some(app), Ok(mut form)) = (weak.upgrade(), fr.try_borrow_mut()) {
          match crate::pipe::reload::notify_reload() {
              Ok(()) => form.show_toast("已发送重载通知"),
              Err(_) => form.show_toast("主进程未运行，无法重载（重启 lyric-for-musicfox 后生效）"),
          }
      }
  });
  ```

**验证**：手动 + 单元测试 `form.show_toast` 接到 `Err` 时是否设置正确文本。

**预估工作量**：1 小时。

---

### 1.3 托盘菜单底部边界保护

**影响**：1080p 右下角右键菜单溢出屏幕。

**改动**：

- `src/platform/windows/mod.rs::show_tray_popup_menu_for_window` 增补：
  - `MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST)` + `MONITORINFO`；
  - 估算菜单高度 `items_count * GetSystemMetrics(SM_CYMENU) + 4`；
  - 如果 `pt.y + menu_height > mi.rcWork.bottom`，则 `y = mi.rcWork.bottom - menu_height`。
- 移除"以鼠标 Y 为准"的隐含假设，让菜单自适应屏幕。

**验证**：手动在 1080p / 1440p / 4K 下分别测右下角 / 右中部 / 顶部鼠标右键，肉眼检查菜单完整可见。

**预估工作量**：2~3 小时。

---

### 1.4 移除 `ScrollView`、调整窗口尺寸

**影响**：滚动条体验；与 BUG-04 修复承诺不一致。

**改动**：

- `ui/settings.slint:130` `ScrollView { ... }` 替换为 `VerticalLayout { ... }`。
- 同步调整：`preferred-height: 520px` → `680px`；`min-height: 360px` → `600px`。
- 验证 `ui/settings.slint` 总内容高度 ≤ 680px，否则再压紧卡片 padding / spacing。

**验证**：手动设置窗口 680px 高 → 不出现滚动条。

**预估工作量**：1~2 小时（含测试回归 14 项功能）。

---

### 1.5 修正 go-musicfox 卡片顺序

**影响**：req §6.2.1 文案一致性。

**改动**：

- `ui/settings.slint` 把卡片 3（`go-musicfox`）与卡片 4（`系统与网络`）的 `Rectangle` 块顺序互换。

**验证**：肉眼对照 req §6.5 验收列表第 1 项。

**预估工作量**：5 分钟。

---

## 批次 2：重要（P1，dead code / 资源 / 文档）

### 2.1 删除三份重复定义与死代码

**改动**：

- 删除 `src/tray/windows_tray.rs`（203 行，dead module）。
- 删除 `src/tray/stub.rs`（20 行，dead module）。
- 删除 `src/services/tray.rs`（30 行，未被调用的 `TrayService`）。
- `src/services/mod.rs::ServiceHandles` 删除 `pub tray` 字段及其初始化。
- `src/lib.rs` 检查是否需要 `pub mod tray` 保留（保留，但只导出 `mod.rs` 即可）。

**验证**：`grep -rn "tray::windows_tray\|tray::stub\|services::tray\." src/` 必须为 0 命中。

**预估工作量**：30 分钟。

---

### 2.2 `on_exit` 调 `shutdown_wt`

**改动**：

- `src/window/mod.rs:442-451` `on_exit` 把 `_services: &ServiceHandles` 改回 `services: &ServiceHandles`（去掉下划线）。
- 在 `on_exit` 末尾调用 `services.wt.shutdown(&ctx.get_config().wt.title)`。

**验证**：起 lyric → 起 WT → 通过托盘 Quit 退出 → 验证 WT 窗口关闭、`go-musicfox` 子进程被 WT 关停；下次启动 lyric → init_delayed_check 还能正常 install 钩子。

**预估工作量**：15 分钟。

---

### 2.3 `install_wt_minimize_hook` 加进程名校验

**改动**：

- `wt_minimizestart_proc` 在标题匹配后，再 `GetWindowThreadProcessId(hwnd, &mut pid)` → `GetModuleFileNameW(NULL, ...)` 或 `QueryFullProcessImageNameW` → 校验包含 `WindowsTerminal.exe`。
- 多实例场景：用 `OnceLock<Mutex<HashSet<isize>>>` 跟踪已托管 HWND，仅处理集合内。

**验证**：手测多 WT 实例 + 误名同子串窗口。

**预估工作量**：2 小时。

---

### 2.4 文档同步

**改动**：

- `docs/dev-stage-3/dev.md` §3 P1 Step 3 删"附灰色说明文本"（当前实现已删）。
- `docs/dev-stage-3/dev.md` §4 表格删 `calculate_safe_menu_pos`（不存在该函数）。
- `docs/dev-stage-3/dev.md` §6 §P2 Step 1 改预览高度为 60px（与实现一致）。
- `docs/dev-stage-3/dev.md` §P4 Step 2 改卡片顺序描述为"在「系统与网络」卡片之后"。
- `docs/dev-stage-3/req.md` §3.2.4 保留说明（当前 UI 没有，但需求要求保留）：或同步改实现加说明行。

**预估工作量**：1 小时。

---

## 批次 3：体验 / 一致性（P2，可下个迭代）

### 3.1 拆分 `platform/windows/mod.rs`

**改动**：拆为 `pipe.rs` / `tray.rs` / `wt.rs` / `monitor.rs` / `wndproc.rs` / `mod.rs`，每个 ~150 行。

**预估工作量**：半天。

---

### 3.2 `PlatformPipeClient::send` 返回 `AppError`

**改动**：

- `src/platform/trait.rs::PlatformPipeClient::send` 改 `Result<Vec<u8>, AppError>`。
- 错误映射：`ERROR_PIPE_BUSY` / `ERROR_FILE_NOT_FOUND` / 超时 → `AppError::PipeConnectionFailed`。

**预估工作量**：2 小时。

---

### 3.3 Slint 预览区封装 `PreviewText` 子组件

**改动**：

- `ui/settings.slint` 把 8 个描边 `Text` + 1 个主 `Text` 封装为 `component PreviewText { in property <string> text; in property <length> font-size; ... }`，9 个节点减少为 1 行实例化。

**预估工作量**：1 小时。

---

### 3.4 补 4 项单元测试

| 测试 | 覆盖 req |
|---|---|
| `resolve_musicfox_data_dir_4_levels` | req §6.3 |
| `validate_musicfox_path_*` | req §5.5、§6.5 |
| `parse_pos_response_partial` | req §6.5 / F05 |
| `notify_reload_returns_err_on_no_main` | req §2.2 / F10 |

**预估工作量**：半天。

---

### 3.5 ShellExecuteW 替代 explorer.exe 子进程

**改动**：`src/platform/windows/mod.rs::reveal_in_file_manager` 与 `launch_wt` 改用 `ShellExecuteW`，避免企业环境下 explorer 不可用。

**预估工作量**：1 小时。

---

## 批次 4：可选清理（不阻塞发布）

- 注释清理（删除 dev.md 中已废弃的 old behavior 描述）。
- 把 `std::env::set_var("SLINT_STYLE", "material")` 统一到 `lib.rs`。
- `clippy.toml` + `cargo clippy --all-targets` 跑一轮。
- 给 dev.md 增加"已知缺陷 / 偏离 req"清单章节，与本 review 文件交叉链接。

---

## 工作量合计

| 批次 | 估时 |
|---|---|
| 批次 1（P0） | ~10 小时 |
| 批次 2（P1） | ~5 小时 |
| 批次 3（P2） | ~12 小时 |
| 批次 4（清理） | ~3 小时 |
| **合计** | **~30 小时 ≈ 4 个工作日** |

适合拆为 2~3 个 PR：

- **PR-1**（P0）：恢复 IPC + 加重载按钮 + 修 ScrollView + 卡片顺序；
- **PR-2**（P0 边界 + P1 死代码）：托盘菜单边界 + 删死代码 + on_exit 调 shutdown_wt；
- **PR-3**（P1 文档 + P2）：WT minimize 进程名校验 + 文档同步 + 拆分 platform/windows/mod.rs + 补测试。