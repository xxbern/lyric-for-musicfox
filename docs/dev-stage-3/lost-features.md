# Stage-3 丢失功能清单（gemini-git-checkout 后）

> 维护人：dev
> 生成时间：working tree 现状审计（git HEAD = a893e28，`dev` 分支）
> 上游设计：`docs/dev-stage-3/req.md`、`docs/dev-stage-3/dev.md`
> 上一份完整 review：`docs/dev-stage-3/review-finding.md`（HEAD 之前已生成）
>
> **背景**：项目 working tree 被某个 gemini 模型错误 `git checkout` 后，
> `ui/settings.slint` 等核心 UI 文件被回退到了 stage-2 末期的「Slint material 默认模板」，
> 大量 stage-3 P0~P4 的视觉/交互功能在 UI 层丢失；但 Rust 侧（`src/`）
> 多数基础设施仍在。本清单**只列丢失项**，不列修复方案（避免越界做决定）。

---

## 0. TL;DR

| 维度 | 状态 | 一句话总结 |
|---|---|---|
| `ui/settings.slint` 现代化视觉 | 🔴 **严重退化** | 退回到 stage-2 material 模板（288 行，无卡片、无预览、无 go-musicfox 卡片、无重载按钮、无端口只读、无字段错误红字） |
| P0 重载语义（差异化 toast） | 🔴 **功能丢失** | 设置 UI 没有"重载歌词"按钮；Rust 侧 `notify_reload()` 仍是 unit 类型，仅在保存路径触发"杀旧拉新"重启，**完全没走 IPC 管道** |
| P1 输入框规范化 | 🔴 **部分丢失** | 端口字段未做只读禁用；端口后灰色提示"端口变更需重启主进程生效" 不存在 |
| P1/P2 实时歌词预览 | 🔴 **功能丢失** | `ui/settings.slint` 没有预览组件（`Rectangle #d4dce7` + "天青色等烟雨" 区域）；仅 `in-out property <brush> preview-text-color / preview-outline-color` 属性挂着，没有渲染节点；用户调整字号/颜色时无任何可视化反馈 |
| P4 设置界面 go-musicfox 卡片 | 🔴 **功能丢失** | UI 上完全没有「🎵 go-musicfox」卡片（无路径输入、无浏览按钮、无打开数据目录按钮、无路径只读提示）；仅 Slint 端预留了 2 个属性，`src/settings/mod.rs` 里的回调 `on_browse_musicfox_clicked / on_open_musicfox_data_clicked` 因此编译期就**根本走不到**（Slint UI 未声明 → 编译时 callback 报错，但当前 `ui/settings.slint` 也跟着把这两 callback 删了，所以编译能过，但运行时永远不触发） |
| P0 托盘菜单 Y 轴安全弹出 | 🔴 **未实施** | `src/platform/windows/mod.rs:240` 仍用 `TPM_TOPALIGN \| TPM_LEFTALIGN` + 原始 `pt.y`，**没有**按 `GetMonitorInfoW(MONITORINFO.rcWork)` 做"菜单顶部 ≤ 鼠标 Y 且底部 ≤ 屏幕高"的安全位移；`req §1.2 第 4 条`不达标 |
| 死代码清理 | 🟠 **未执行** | `src/services/tray.rs`（TrayService，无调用点）仍存在；`tests/mock_udp_sender.rs` 被**误删**（跑到 `src/bin/mock_udp_sender.rs` 去了），违背原 P1 阶段把它拆出去的目的 |
| F10 重载反馈端到端测试 | 🟠 **空缺** | `tests/settings_test.rs` 中无任何"在线 toast / 离线 toast"断言（与上一份 `review-finding.md` §P1-11 完全一致，没有任何改进） |
| WT 托管 / P3 / go-musicfox 落盘 | 🟢 **基本保留** | `src/services/wt.rs`、`src/config/mod.rs::WtConfig`、`src/path.rs::resolve_musicfox_data_dir`、`src/platform/windows/mod.rs::PlatformWt` 全套都还在；与 UI 丢失正相反，Rust 侧实现完整 |
| 歌词悬浮窗 UI | 🟢 **保留** | `ui/lyric.slint`（172 行）仍是 stage-3 完成态：8 向描边、透明背景、滚动状态机、拖拽 TouchArea 全部在 |

---

## 1. 用户视觉直接感知的退化（设置窗口）

下面这一节是用户在 1080p 双击 `lyric-for-musicfox.exe --settings` 立刻看得到的差异。

### 1.1 🔴 设置窗口退回到 stage-2 模板

| # | 丢失项 | 对应设计 | 当前 `ui/settings.slint` 现状 | 用户感知 |
|---|---|---|---|---|
| V1 | **没有现代 Fluent 卡片化背景** | `req §7.2` + `dev.md §P1 Step 1`：窗口背景 `#f3f4f6`、4 个白底卡片（`#ffffff` + `border-radius: 8px` + `border-color: #e5e7eb`） | 仅 2 个 `Rectangle`（底部操作栏 + Toast + 2 个 modal），其余全是裸 `Text` / `LineEdit` 平铺在白底上 | 整窗一片白，"打字稿"样界面 |
| V2 | **没有「🪟 窗口与定位」卡片容器** | `dev.md §P1 Step 2`：`Rectangle { background: #ffffff; border-radius: 8px; ... }` 包住窗口尺寸 + 位置 + 置顶 + 锁定 | 这 4 个字段只是用 4 条独立 `HorizontalLayout` 拼在主 `VerticalLayout` 里，没有白底容器 | 区块之间没有视觉分隔 |
| V3 | **没有「🎨 歌词与样式」卡片容器** | 同上 | 同 V2 | 同 V2 |
| V4 | **没有「⚙️ 系统与网络」卡片容器** | 同上 | 同 V2 | 同 V2 |
| V5 | **没有「🎵 go-musicfox」卡片（P4）** | `req §6.2.1`：4 号卡片，含路径输入 + 浏览… + 打开数据目录 + 只读提示 | **整个卡片不存在**；仅留 2 个挂着的 in-out 属性 `wt-musicfox-path-text`、`musicfox-data-dir-hint`，UI 上无任何渲染节点 | 用户在 UI 上无法设置 WT 启动路径；`wt_musicfox_path` 永远是 `Config::default()` 给的 `C:\Users\xx\app\musicfox\musicfox.exe` |
| V6 | **没有「歌词调整预览」区域（P2）** | `req §4`：高度 80px，背景 `#d4dce7`，固定文字"天青色等烟雨"，实时联动字号/字体/颜色/描边；`#d4dce7` / `天青色等烟雨` 在文件中 grep 不到 | 仅 `in-out property <brush> preview-text-color / preview-outline-color` 占位，**无 Rectangle 也无 Text 节点** | 调整字号/颜色时用户**看不到任何反馈** |
| V7 | **没有 ScrollView**（`P0-P3-bug.md` §3 要求移除外层 ScrollView） | 已符合 | 已符合 | OK（这是少数几项 working tree 实际按 bug 报告修了的部分） |

### 1.2 🔴 底部操作栏退回到 2 按钮

| # | 丢失项 | 对应设计 | 当前 `ui/settings.slint` 现状 |
|---|---|---|---|
| B1 | **没有「🔄 重载歌词」按钮** | `req §2.2.1` + `dev.md §P0 Step 3`：独立按钮，发送 reload IPC，根据 Result 弹差异化 toast |
| B2 | **没有「💾 保存并应用」主色高亮样式** | `dev.md §P1 Step 4`：底色 `#2563eb` + 白字的主按钮 |
| B3 | **没有 per-field 红字校验错误** | `req §6.2.3`：`field_errors` 机制，每个字段下方红字 | 仅底部统一 `save-error` 一行红字；用户保存失败时**不知道是哪个字段错** |

### 1.3 🔴 端口字段未做只读规范化

| # | 丢失项 | 对应设计 | 当前 `ui/settings.slint` 现状 |
|---|---|---|---|
| P1 | 端口 `LineEdit` 未设置 `enabled: false` | `req §3.2.1`：只读禁用，避免误导用户 |
| P2 | 端口字段后无灰色说明文字"端口变更需重启主进程生效；此处只读" | `req §3.2.4` + `dev.md §P1 Step 3` | 完全缺失 |

### 1.4 🟠 卡片顺序与提示细节丢失

| # | 丢失项 | 当前状态 |
|---|---|---|
| O1 | 即使将来加回 go-musicfox 卡片，**应放在「系统与网络」之后**（`req §6.2.1`） | 当前无 4 号卡片，谈不上顺序；但恢复时必须按规范 |
| O2 | Toast 视觉现代化（`req §7.2.6`：深色半透明胶囊 + 圆角） | 当前 Toast 是 `#dd333333` 半透明 + `border-radius: 6px`，**勉强能用**，但 dev.md §P0 设计稿要求更精致的胶囊样式 |

---

## 2. 用户看不见但功能断裂项（Rust ↔ UI 联动）

### 2.1 🔴 P0 重载语义退回到 stage-2 行为

**核心问题**：stage-3 的 P0 设计要求把"重载歌词"做成 IPC 通知 + 差异化 toast 反馈，但当前实现是：

1. `src/pipe/mod.rs:14-22` 的 `notify_reload()` 是 unit 类型，**根本不是 `Result<(), AppError>`**：
   ```rust
   pub fn notify_reload() {
       #[cfg(windows)]
       crate::platform::windows::restart_lyric_app();   // 杀旧拉新！
       #[cfg(not(windows))]
       crate::platform::stub::restart_lyric_app();
   }
   ```
2. `src/settings/form.rs:152-156` 在 `flush_and_save_now` 末尾调 `notify_reload()`，等价于每次保存配置都杀旧主进程 + 250ms 后拉新主进程，**完全不写 reload-{session_id} 命名管道**。
3. UI 上根本没有"重载歌词"按钮（见 §1.2 B1），因此 `req §2.3 验收表`中两行场景：
   - 主进程运行 → toast "已发送重载通知"
   - 主进程未运行 → toast "主进程未运行，无法重载（重启 lyric-for-musicfox 后生效）"

   **当前代码路径全部走不到**——这两个文案在 `src/` 和 `ui/` 中 grep 命中数 = 0。

| # | 丢失项 | 对应设计 | 后果 |
|---|---|---|---|
| R1 | `notify_reload()` 返回类型 | `dev.md §P0 Step 2`：`pub fn notify_reload() -> Result<(), crate::error::AppError>` | 当前 `()`，无法表达"主进程是否在线"语义 |
| R2 | `notify_reload()` 实际行为 | `dev.md §P0 Step 2`：写 `IpcMessage::ReloadConfig` 到 `reload-{session_id}` 命名管道 | 当前 = `restart_lyric_app()`（杀旧拉新） |
| R3 | 设置 UI「重载歌词」按钮 | `req §2.2.1` | 完全缺失 |
| R4 | 设置 UI 重载回调 on_reload_clicked | `dev.md §P0 Step 3` | 完全缺失 |
| R5 | "已发送重载通知" Toast | `req §2.2.3` | 文案不存在；grep 0 命中 |
| R6 | "主进程未运行，无法重载（重启 lyric-for-musicfox 后生效）" Toast | `req §2.2.3` | 文案不存在；grep 0 命中（注意：`src/settings/mod.rs:573` 的灰色提示 "主进程未运行或已退出" 是位置拉取降级文案，**不是**重载 toast，二者不能混用） |
| R7 | `PlatformPipeClient::send` 在 `ERROR_FILE_NOT_FOUND` 时返回 `AppError::PipeConnectionFailed` | `dev.md §P0 Step 2` 要求 | 当前 trait 返回 `Result<Vec<u8>, String>`，无结构化错误 |

### 2.2 🔴 F08 校验反馈无法定位字段

`req §6.2.3` + `req §8.2 F08` 要求"字段下方提供校验错误红字（沿沿用现有 `field_errors` 机制）"。当前：

- UI 没有 per-field 红字节点
- Slint 没有 `in-out property <string> musicfox-path-error` 等字段级错误属性
- `src/settings/mod.rs::on_save_clicked` 把所有错误拼成一行 `"保存失败：width: ...；musicfox_path: ..."` 塞到 `save-error`，用户**不知道具体哪个字段错**

### 2.3 🔴 P4 浏览与打开数据目录按钮回调整

`src/settings/mod.rs` 中存在完整 Rust 实现：
```rust
// on_browse_musicfox_clicked: rfd FileDialog
// on_open_musicfox_data_clicked: resolve_musicfox_data_dir + reveal_in_file_manager
```
但 `ui/settings.slint` **完全没声明 `browse-musicfox-clicked` 与 `open-musicfox-data-clicked` 回调节点**，用户**根本点不到**。

注意：之前 `ui/settings.slint` 是包含这两个 callback 的，working tree 把它们连同整个 🎵 go-musicfox 卡片一起删掉了，于是 `src/settings/mod.rs::run()` 中 `app.on_browse_musicfox_clicked(...)` 的绑定成了一个**无效绑定**——Slint 编译时如果 UI 端没声明 callback，Rust 侧 `on_xxx` 方法不会被生成；绑定代码会**编译失败**。当前 working tree `src/settings/mod.rs` 还在调用这些 `on_browse_musicfox_clicked`，意味着只要 cargo build 跑过、UI 端必然也有 callback 声明——但 audit 时 `ui/settings.slint` 文件中**只有 `browse-musicfox-clicked` / `open-musicfox-data-clicked` 两个 callback 出现在底部操作栏里漂浮的注释行**，并不是绑在有效控件上——这是 audit 看到的真实情况，可能是「最后一次 cargo build 跑的版本」与「working tree」之间存在 git 改动但 build 未验证。

### 2.4 🟠 死代码模块清理不彻底

`docs/dev-stage-3/review-finding.md §P1-1` 已记录的死代码问题仍然在：
- `src/services/tray.rs`（TrayService，无任何 `services.tray.foo()` 调用点，`grep -rn "services\.tray" src/` 仅 `mod.rs` 自指）
- `src/bin/mock_udp_sender.rs` 是新拆分目的位置，但 `tests/mock_udp_sender.rs` 被 `git status` 标记为 `deleted` —— 测试入口没了

### 2.5 🟠 F11~F14 验收被破坏

| # | 丢失项 | 当前状态 |
|---|---|---|
| T1 | F10 重载反馈端到端测试 | 无；`tests/settings_test.rs::toast_lifecycle_visible_and_expires` 仅断言字符串字面量"已发送重载通知"，但用户 UI 入口根本走不到 |
| T2 | F11 字体回退告警 | UI 字段级错误位置缺失，**回退告警目前只能落到底部 save-error**（破坏 §2.2） |
| T3 | F13 未保存关闭拦截 | 仍可用（`show-close-modal` 保留） |

---

## 3. P0 托盘交互缺陷（与 UI 无关，但属于本阶段丢失）

### 3.1 🔴 托盘菜单底部仍可能溢出屏幕

`src/platform/windows/mod.rs:240-245` 的 `TrackPopupMenuEx` 调用：
```rust
TrackPopupMenuEx(
    hmenu,
    (TPM_TOPALIGN | TPM_LEFTALIGN | TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
    pt.x,
    pt.y,                 // ← 原始 pt.y，未做边界检测
    target_hwnd,
    None,
);
```
未与 `GetMonitorInfoW(MONITORINFO.rcWork)` 做"菜单底部 ≤ screen_bottom"的安全位移。

`req §1.2 第 4 条` 要求"鼠标在屏幕任意 Y 位置按下右键，菜单顶部边界 ≤ 鼠标 Y，且菜单底部边界 ≤ 屏幕物理像素高度"——**当前实现只满足前者**。

`dev.md §P0 Step 1` 描述了标准做法（`MonitorFromPoint` + `GetMonitorInfoW` + 估算菜单高度），未实施。

### 3.2 🟡 菜单弹出位置容错

`GetCursorPos` 失败时 `pt = POINT::default() = (0,0)`，会把菜单弹到屏幕左上角（`src/platform/windows/mod.rs:204-206`，见 `review-finding §P2-2`）。

---

## 4. 与上游 review 文档的对照

`docs/dev-stage-3/review-summary.md` 与 `review-finding.md` 是 **stage-3 完成态的 review** 报告，记录了 stage-3 实现相对 req.md/dev.md 的差距。**本次丢失功能项 = working tree 状态相对 review 报告的退化**：

| Review 中的项 | working tree 实际状态 |
|---|---|
| P0-1 设置 UI 没有"重载歌词"按钮 | 🔴 **更糟** —— 不仅按钮没，连 callback 节点都没（review 时 callback 还有，只是没绑按钮） |
| P0-2 `notify_reload` 是 unit | 🔴 **未修复** —— 仍 unit + 仍是 `restart_lyric_app()` |
| P0-3 托盘菜单底部溢出 | 🔴 **未修复** |
| P0-4 卡片顺序（go-musicfox 应在系统与网络后） | 🔴 **更糟** —— go-musicfox 卡片整张没了，谈不上顺序 |
| P0-5 `ScrollView` 残留 | 🟢 **已修**（commit bdd2063）—— 但工作区中其实目前 settings.slint 也没 ScrollView |
| P1-1 三份 TrayCmd + 死代码 | 🟡 **部分修** —— `tray/windows_tray.rs` 与 `tray/stub.rs` 已删（commit 344e44f），但 `services/tray.rs` 还在 |
| P1-2 ShowContextMenu 同步阻塞 | 🟡 **未修复** |
| P1-3 install_display_change_hook WndProc 子类化 | 🟡 **未修复** |
| P1-4 全局 WinEventHook 误命中 | 🟡 **未修复** |
| P1-5 WtService toggle 反复读盘 + 锁竞争 | 🟡 **未修复** |
| P1-6 ComboBox 双源不一致 | 🟡 **未修复** |
| P1-7 非 Windows 下 musicfox_path 怪异 C: 判断 | 🟡 **未修复** |
| P1-8 错误提示不能定位字段 | 🔴 **更糟** —— field_errors 机制整体缺失 |
| P1-9 描边宽度单位 | 🟡 **未修复** |
| P1-10 主进程退出未 shutdown_wt | 🟡 **未修复** |
| P1-11 toast 测试只字面量 | 🟡 **未修复** |
| P1-12 update_preview_brushes 重复 | 🟡 **未修复** |

**结论**：working tree 相对 review 报告的状态在 5 个关键项上是"更糟"（UI 整体丢失），其他项保持原状未修。

---

## 5. 文件级 audit 摘要

| 文件 | working tree 状态 | 说明 |
|---|---|---|
| `ui/settings.slint` | 288 行，严重退化 | 退回到 stage-2 material 模板；缺失 4 卡片 / 预览 / go-musicfox 卡 / 重载按钮 / 端口只读 / 字段错误红字 |
| `ui/lyric.slint` | 172 行 | stage-3 完成态，保留 |
| `ui/lib.slint` | （未读） | （未读，dev.md 未涉及） |
| `src/settings/mod.rs` | 包含 stage-3 完整逻辑 | browse / open-data / preview brushes / 完整绑定；但回调无 UI 节点可绑 |
| `src/settings/form.rs` | 包含 stage-3 完成态 | 防抖 / 解析失败 modal / toast 调度 / normalize |
| `src/settings/validate.rs` | 包含 `validate_musicfox_path` | 与 req §6.2.3 对齐 |
| `src/config/mod.rs` | 包含 WtConfig + 4 个字段默认值 | 与 req §5.5 对齐 |
| `src/path.rs` | 包含 `resolve_musicfox_data_dir` 4 级 fallback | 与 req §6.3 对齐 |
| `src/services/wt.rs` | 包含 toggle / init_delayed_check / shutdown | 与 dev.md §P3 对齐 |
| `src/services/tray.rs` | 死代码 | 见 §2.4 |
| `src/services/{config,font,monitor,position,style,udp}.rs` | （未深入读，未在丢失清单） | |
| `src/pipe/mod.rs` | `notify_reload` 是 unit + 重启 | 见 §2.1 |
| `src/platform/trait.rs` | 包含 `PlatformWt` | 完整 |
| `src/platform/windows/mod.rs` | 包含 `PlatformWt` 实现 / `show_tray_popup_menu_for_window` | 见 §3 托盘菜单缺陷 |
| `src/platform/stub/mod.rs` | 实现 trait | |
| `src/main.rs` | 不再启动 WT（与 req §5.3 对齐） | OK |
| `src/tray/mod.rs` | 仅 `TrayCmd` + init / flash | 已清理过（commit 344e44f） |
| `src/window/mod.rs:419-451` | Quit 路径调 `services.wt.shutdown` | OK（review P1-10 是说 on_exit 没调，Quit 路径已调） |
| `src/bin/mock_udp_sender.rs` | 新位置（拆分目的） | OK |
| `tests/mock_udp_sender.rs` | **已删除**（git status 标 deleted） | 丢失 |

---

## 6. 验收矩阵（来自 req.md，对照当前状态）

| 序号 | 验收项 | req.md 章节 | 当前 working tree 状态 |
|---|---|---|---|
| 1.4 | 1080p 右下角托盘右键菜单完整可见 | `§1.3` | 🔴 仅满足"顶部 ≤ 鼠标 Y"，底部仍可能溢出 |
| 2.3 | 主进程在线 → toast "已发送重载通知" | `§2.3` | 🔴 无 UI 入口，无 toast 文案 |
| 2.3 | 主进程离线 → toast "主进程未运行，无法重载..." | `§2.3` | 🔴 同上 |
| 3.1 | 宽/高无 ± 控件 | `§3.1` | 🟢 当前已是 LineEdit（但缺禁数字过滤在 UI 层，需 Rust `parse_digits_u32` 兜底） |
| 3.2 | 端口只读 + 灰色说明 | `§3.2` | 🔴 未禁用，未加说明 |
| 4.1 | 预览区域背景 `#d4dce7` + "天青色等烟雨" | `§4` | 🔴 整块预览缺失 |
| 4.3 | 实时跟随字号/颜色/字体/描边 | `§4.2` | 🔴 预览缺失 |
| 4.1.7 | 预览不触发脏标记 | `§4.1.7` | 🔴 预览不存在，谈不上脏标记 |
| 5.6 | 主进程退出关 WT + 清理钩子 | `§5.6` | 🟡 Quit 路径已调 shutdown；on_exit 路径未调（review P1-10） |
| 6.2 | 「🎵 go-musicfox」卡片含路径 + 浏览 + 打开数据目录 | `§6.2` | 🔴 整张卡片缺失 |
| 6.2.3 | 字段下方红字校验错误 | `§6.2.3` | 🔴 字段红字机制缺失 |
| 6.3 | DataDir 4 级 fallback 解析 | `§6.3` | 🟢 Rust 实现完整 |
| 6.5 | 保存后 config.toml 含 `[wt]` | `§6.4` | 🟢 默认值正确（待验证） |
| 7.2 | 4 卡片化 + 主按钮高亮 + 模态美化 | `§7.2` | 🔴 仅 2 个 modal 样式简单白卡，4 卡片整体缺失 |
| F01 | 正常配置加载 | `§8.2 F01` | 🟢 bootstrap 路径完整 |
| F02 | 首次运行默认创建 | `§8.2 F02` | 🟢 同上 |
| F03 | 临时副本恢复 | `§8.2 F03` | 🟢 同上 |
| F04 | 配置文件损坏恢复 | `§8.2 F04` | 🟢 解析失败 modal + 重置链路完整 |
| F05 | 实时位置拉取 | `§8.2 F05` | 🟢 |
| F06 | 位置离线降级 | `§8.2 F06` | 🟢 灰色提示存在 |
| F07 | 防抖暂存 | `§8.2 F07` | 🟢 500ms debounce |
| F08 | 字段合法性校验拦截 | `§8.2 F08` | 🟡 校验逻辑完整，**但 UI 无法定位具体字段** |
| F09 | 保存并实时热重载 | `§8.2 F09` | 🔴 **退化为"杀旧拉新"重启**，违背"原子写 + IPC 通知" |
| F10 | 重载歌词独立反馈 | `§8.2 F10` | 🔴 无 UI 入口，无差异化 toast |
| F11 | 字体族枚举 | `§8.2 F11` | 🟢 ComboBox + GDI 枚举 |
| F12 | 打开配置目录 | `§8.2 F12` | 🟢 |
| F13 | 未保存关闭拦截 | `§8.2 F13` | 🟢 |
| F14 | 单实例与激活 | `§8.2 F14` | 🟢 |

---

## 7. 用户视角总结（最容易感知的 4 项）

1. **打开设置窗口像回到 stage-2**：一片白底平铺、没有卡片、没有预览、没有 go-musicfox 卡片、底部只有 2 个按钮（缺"重载歌词"）。
2. **调整字号/颜色/字体看不到效果**：因为预览组件整个没了。
3. **保存后主进程会被杀 + 重启**：而不是上一版 P0 设计的"无缝 IPC 重载"。
4. **右下角托盘右键菜单可能仍然会被屏幕底边吃掉几像素**：在 1080p 屏幕上恰好够装，但在 2K/4K + DPI 缩放下高概率溢出。

---

## 8. 修复优先级建议（仅记录，不实施）

| 优先级 | 项 | 估时 |
|---|---|---|
| P0 | 把 `ui/settings.slint` 恢复为 stage-3 完成态（含 4 卡片 + 预览 + 重载按钮 + 端口只读 + 字段红字） | ~3 小时 |
| P0 | 恢复 `notify_reload() -> Result<(), AppError>` 签名 + 写 reload-{sid} 管道 + settings UI 绑定 on_reload_clicked + 差异化 toast | ~1 小时 |
| P0 | 修复 `TrackPopupMenuEx` 的 Y 轴安全位移 | ~30 分钟 |
| P1 | 删除 `src/services/tray.rs` 死代码 | ~10 分钟 |
| P1 | 把 `tests/mock_udp_sender.rs` 恢复（如果 P1 拆分目的要求两边都有） | ~5 分钟 |
| P2 | review-finding.md 中的 P1-2 ~ P1-12 整改 | ~半天 |

---

## 9. 附录：审计命令清单（可重放）

```bash
# UI 现状
wc -l ui/settings.slint ui/lyric.slint

# Rust 侧关键状态
grep -c "已发送重载通知\|主进程未运行，无法重载" src/ ui/ -r
grep -n "reload-clicked\|on_reload" src/settings/mod.rs ui/settings.slint -r
grep -n "ScrollView\|🎵.*go-musicfox\|#d4dce7\|天青色等烟雨" ui/settings.slint

# 死代码 / 拆分完整性
ls src/tray/                            # 应仅 mod.rs
grep -rn "services\.tray\." src/         # 应 0 命中
ls tests/mock_udp_sender.rs src/bin/mock_udp_sender.rs  # 注意 working tree 删除前者

# P0 重载语义
grep -n "pub fn notify_reload" src/pipe/mod.rs
grep -n "restart_lyric_app\|restart_lyric" src/ -r

# P0 托盘菜单弹出
grep -n "TrackPopupMenuEx\|MonitorFromPoint\|MONITORINFO\|menu_height" src/platform/windows/mod.rs
```