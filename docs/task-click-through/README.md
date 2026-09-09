# 歌词窗口任务栏与系统托盘穿透任务规划

> **分支**：`fix/click-through`  
> **状态**：待执行（Ready to implement）  
> **关联议题**：当歌词窗口位于屏幕底部状态栏/系统托盘上方时，鼠标无法穿透，托盘图标被遮挡且点击失效。

---

## 一、 问题背景与根因分析

在锁定状态（`locked = true`）下，用户将歌词窗口移动或贴靠在屏幕右下角任务栏、系统托盘区域时，鼠标交互无法穿透歌词窗口，导致无法悬停显示托盘图标提示、无法点击托盘图标弹出菜单。

### 核心根因：
1. **缺少 `WS_EX_LAYERED`（Win32 穿透硬性前提）**：
   - Windows 官方 DWM 规范明确要求：`WS_EX_TRANSPARENT`（点击穿透样式）仅在窗口同时具备分层样式 `WS_EX_LAYERED` 时才被系统命中测试引擎支持。
   - 当前 `src/platform/windows/mod.rs` 的 `apply_locked_style` 仅添加了 `WS_EX_TRANSPARENT`，未注入 `WS_EX_LAYERED`。
2. **缺少 `WM_NCHITTEST -> HTTRANSPARENT` 消息拦截（特权窗口穿透关键）**：
   - 任务栏（`Shell_TrayWnd`）和托盘区（`TrayNotifyWnd`）为 Windows Explorer 特权系统窗口。
   - 彻底穿透至特权窗口的工业标准做法是：在 `WndProc` 中拦截 `WM_NCHITTEST (0x0084)`，并在锁定态下直接返回 `HTTRANSPARENT (-1)`。
   - 当前 `monitor_wnd_proc` 未拦截 `WM_NCHITTEST`。

---

## 二、 技术改造方案

1. **分层扩展样式注入**：
   - 在 `apply_locked_style` 中：
     - `locked = true`：同时赋予 `WS_EX_TRANSPARENT` 与 `WS_EX_LAYERED`；
     - `locked = false`：移除 `WS_EX_TRANSPARENT`，恢复正常拖拽点击。
2. **窗口过程拦截命中测试**：
   - 在 `monitor_wnd_proc` 中拦截 `msg == 0x0084` (`WM_NCHITTEST`)：
     - 若当前窗口处于 `locked = true`，直接返回 `LRESULT(-1)`（`HTTRANSPARENT`）；
     - 若未锁定，透传给 `CallWindowProcW` 正常处理。
3. **自动化测试与验收**：
   - 编写 `tests/click_through_test.rs` 测试锁定/未锁定的状态样式流转契约。
   - 通过 subagents `reviewer` 审查代码。

---

## 三、 执行任务清单

- [ ] **Task 1**：修改 `src/platform/windows/mod.rs` 中的 `apply_locked_style`，注入 `WS_EX_LAYERED` 分层窗口样式。
- [ ] **Task 2**：在 `monitor_wnd_proc` 中增加 `WM_NCHITTEST` 消息拦截，锁定态返回 `HTTRANSPARENT (-1)`。
- [ ] **Task 3**：在 `tests/click_through_test.rs` 编写对应的单元/集成测试。
- [ ] **Task 4**：运行全量回归测试 `cargo test`。
- [ ] **Task 5**：调用 `subagents reviewer` 专家子代理完成代码审查与闭环交付。
