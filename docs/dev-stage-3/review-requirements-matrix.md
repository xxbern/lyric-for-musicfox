# Stage-3 需求 ↔ 实现对照矩阵

> 来源：`docs/dev-stage-3/req.md` §1 ~ §8、`docs/dev-stage-3/P0-P3-bug.md` §3
> 状态：PASS / PARTIAL / FAIL / MISSING

## §1 托盘右键菜单高度限制

| 验收项 | 期望 | 代码位置 | 状态 |
|---|---|---|---|
| 1.1 顶部 ≤ 鼠标 Y | TPM_TOPALIGN + pt.y | `src/platform/windows/mod.rs:240` TrackPopupMenuEx `(TPM_TOPALIGN \| TPM_LEFTALIGN \| TPM_RETURNCMD \| TPM_RIGHTBUTTON).0, pt.x, pt.y` | **PASS** |
| 1.2 X 轴仍以鼠标 X 对齐 | TPM_LEFTALIGN | 同上 | **PASS** |
| 1.3 顶部 ≤ 鼠标 Y 且 底部 ≤ 屏幕高 | 需做向上位移 / 屏幕边界检测 | 同上 — **未做** | **FAIL** |
| 1.4 验收 1080p 右下角 | — | — | 取决于 1.3 |

> 详见 `review-finding.md` §3.2。
> `P0-P3-bug.md` BUG-01 的根因分析（"tray-icon with_menu 内部拦截右键 → 自动弹菜单 → 绕过应用层 Y 约束"）已经在 stage-3 修复（移除 `with_menu` + 显式 `TrackPopupMenuEx`），但**修复方向对了，仍缺底部边界检测**。

## §2 设置界面"重载"语义修复

| 验收项 | 期望 | 代码位置 | 状态 |
|---|---|---|---|
| 2.2.1 重载按钮复用主进程重载语义 | 设置进程 → 写 reload 管道 → 主进程 reload | `src/settings/mod.rs` **没有"重载歌词"回调** | **FAIL** |
| 2.2.2 失败时 toast 反馈失败原因 | "主进程未运行，无法重载..." | `src/settings/mod.rs:341` 只 `show_toast("配置已保存")` | **FAIL** |
| 2.2.3 成功时 toast "已发送重载通知" | 不使用 "配置已保存并已实时生效" | 无代码 | **MISSING** |
| 2.2.4 主进程收到重载通知的语义不变 | 同 stage-2 §P2 | `src/pipe/mod.rs::reload::notify_reload()` **已变**：直接 `restart_lyric_app()` 而不是写管道 | **FAIL**（语义变了） |
| 2.2.5 保存按钮行为不变 | 原子写 + reload 通知 + toast | `src/settings/form.rs::flush_and_save_now` 原子写 OK；但通知走 `restart_lyric_app()` 而非 IPC | **PARTIAL** |
| 2.2.6 失败 toast 与成功 toast 时长一致 | 3 秒 | `TOAST_DURATION = Duration::from_secs(3)` (`src/settings/form.rs:10`) | **PASS** |
| 2.3.1 主进程运行中 → "已发送重载通知" | — | 无路径 | **MISSING** |
| 2.3.2 主进程未运行 → "主进程未运行..." | — | 无路径 | **MISSING** |
| 2.3.3 托盘右键"重载歌词" | 自身 reload，无 toast | `src/window/mod.rs:400` 调 `ConfigService::load_or_default()` + `ctx.update_config` | **PASS** |

> 当前实现把"保存"和"重载"完全合并到 `restart_lyric_app()` 路径（杀旧拉新），req §2 的差异化 toast 模型根本没建出来。

## §3 输入框调整

| 验收项 | 期望 | 代码位置 | 状态 |
|---|---|---|---|
| 3.1.1 宽 / 高改纯数字文本框 | `CompactInput` (Slint) | `ui/settings.slint:23` `component CompactInput` + 第 200 行使用 | **PASS** |
| 3.1.2 仅允许 `[0-9]` 静默丢弃 | 解析时 filter 数字 | `src/settings/mod.rs:188` `parse_digits_u32` `s.chars().filter(is_ascii_digit)` | **PASS** |
| 3.1.3 空 → 0 → 越界提示 | `parse_digits_u32("")` 返回 0 | 同上 | **PASS** |
| 3.1.4 范围 100..=4000 / 20..=500 | `validate_width/height` | `src/settings/validate.rs:5,11` | **PASS** |
| 3.2.1 端口只读 LineEdit | 控件 enabled=false | `ui/settings.slint` 用 `Text` 文本 (`"接收: " + root.recv-port-text + ...`) **不是 LineEdit** | **PARTIAL**（按 req 3.2.1 要求"改为单纯数字文本框（与宽度 / 高度一致），且不允许编辑"——目前是静态文本，UI 上不可编辑；但不是 LineEdit；dev.md §3.1 Step 3 要求保留灰色说明行，目前也**没有**灰色说明行） |
| 3.2.2 端口只读的原因 | UI 提示 | 无 | **FAIL** |
| 3.2.3 端口值仍按当前 config 显示 | `recv_port_text / send_port_text` | `src/settings/mod.rs:170-171` 来自 `form.draft.system.{receive,send}_port` | **PASS** |
| 3.2.4 端口字段后追加灰色提示行 | "端口变更需重启主进程生效；此处只读" | 无 | **FAIL** |
| 3.3 端口输入验证 | 鼠标点上去光标不变 + 输入无响应 + 灰色提示显示 | 静态 Text 自然无响应；灰色提示缺失 | **PARTIAL** |

## §4 歌词调整预览

| 验收项 | 期望 | 代码位置 | 状态 |
|---|---|---|---|
| 4.1.1 新增预览区 | `preview-rect` Rectangle | `ui/settings.slint:345` | **PASS** |
| 4.1.2 背景 `#d4dce7` | `background: #d4dce7` | 同上 | **PASS** |
| 4.1.3 文本「天青色等烟雨」 | `text: "天青色等烟雨"` | `ui/settings.slint:354` 等 | **PASS** |
| 4.1.4 实时跟随（不依赖保存） | binding to root 属性 | 同上 | **PASS** |
| 4.1.5 水平填满、垂直约 80 DIP | 实际 `60px` | `ui/settings.slint:345` `height: 60px` | **PARTIAL**（60px vs 80px，差 20px；req §4.1.5 写"约 80 DIP"，"约"字给容差，但与 dev.md §P2 Step 1 "高度固定 80px" 不符） |
| 4.1.6 位置在「歌词与样式」卡片底部 | 紧跟描边宽度之后 | `ui/settings.slint:345` 卡片 2 内最后 | **PASS** |
| 4.1.7 不参与脏标记 | 不调用 `form.mark_dirty()` | `ui/settings.slint` `preview-rect` 块内**没有** `root.changed()` 触发 | **PASS** |
| 4.2 样式字段 | family / size / bold / italic / color / outline | `ui/settings.slint:354-462` 8+1 个 `Text` 节点 | **PASS** |

> 注意：dev.md §P2 Step 1 写"字号：绑定 `root.font-size-pt * 1pt`"，但 `lyric.slint` 当前用 `font-size-px`（物理像素）做驱动，与 settings `font-size-pt` 之间没有换算。`window/mod.rs::apply_style_properties` 是 `app.set_font_size_px(cfg.lyric_style.font_size.max(1.0).into())`，**预览和实窗字号单位一致**；这点不算 bug，但要确认 PT vs PX 不混淆。

## §5 go-musicfox WT 终端窗口托管

| 验收项 | 期望 | 代码位置 | 状态 |
|---|---|---|---|
| 5.2.1 取消 music_tray.ahk 中 lyric-for-musicfox.exe 启动 | 不再启动 | `musicfox-patch/build_musicfox_with_plugin.sh` 注释保留 | **PASS**（脚本注释而非 Rust 端） |
| 5.2.2 内嵌 WT 托管 | 主进程启动 1s 后 init_delayed_check | `src/window/mod.rs:130` `services.wt.init_delayed_check(config.wt.clone())` | **PASS** |
| 5.2.3 单击托盘 = 切 WT | 不打开设置 | `src/platform/windows/mod.rs:165-168` Left → `TrayCmd::ToggleWt` | **PASS** |
| 5.2.4 最小化拦截在钩子线程外 | `thread::spawn` 排队 | `src/platform/windows/mod.rs:798-808` `thread::spawn(move \|\| thread::sleep(10ms) → HideWT)` | **PASS** |
| 5.2.5 启动后应用样式 | `apply_wt_hosted_style` | `src/platform/windows/mod.rs:872-905` | **PASS** |
| 5.2.6 alpha=0/255 切显隐，不动 ShowWindow | `SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA)` | `src/platform/windows/mod.rs:907-914` | **PASS** |
| 5.2.7 移除最小化/最大化按钮 | `WS_CAPTION \| WS_SYSMENU \| WS_MAXIMIZEBOX \| WS_MINIMIZEBOX` 清掉 | `src/platform/windows/mod.rs:884-889` | **PASS** |
| 5.2.8 退出清理钩子 + 关闭 WT | `shutdown_wt` | `src/platform/windows/mod.rs:962-976` | **PASS** |
| 5.3 启动顺序 | 主进程启动不启动 WT；+1s 探测 | `src/window/mod.rs:130` | **PASS** |
| 5.4 单击托盘逻辑 | 4 步 | `src/services/wt.rs:35-109` | **PASS**（细节见 `review-finding.md` §3.3） |
| 5.5 配置字段 | wt_musicfox_path / app_dir / title + 校验 | `src/config/mod.rs:227-244`、`src/settings/validate.rs:65` | **PASS** |
| 5.6 验收 4 项 | — | — | 见 `review-finding.md` §3.3 |

## §6 设置界面新增 go-musicfox 配置

| 验收项 | 期望 | 代码位置 | 状态 |
|---|---|---|---|
| 6.2.1 「🎵 go-musicfox」卡片，放在「系统与网络」之后 | 卡片 3 在卡片 4 之前 | `ui/settings.slint:459` 卡片 3（`go-musicfox`），卡片 4 是「系统与网络」 | **FAIL**（**位置反了**，req §6.2.1 写"放置在「系统与网络」卡片**之后**"） |
| 6.2.2 路径 + 浏览按钮 | `CompactInput` + Button "浏览…" | `ui/settings.slint:475-485` | **PASS** |
| 6.2.3 校验错误红字 | 沿用 `field_errors` 机制 | `src/settings/validate.rs:65-79` `validate_musicfox_path` | **PASS**（但 UI 是否在路径字段下显示红字未确认；目前 `save_error` 是统一栏，不一定紧邻 musicfox 字段） |
| 6.2.4 「打开数据目录」按钮 + 4 级 fallback | `resolve_musicfox_data_dir()` | `src/path.rs:18-53` + `src/settings/mod.rs:381-393` | **PASS** |
| 6.2.5 「浏览…」按钮 = 原生文件选择 | `rfd::FileDialog` | `src/settings/mod.rs:361-379` | **PASS** |
| 6.2.6 字段落盘 | `flush_and_save_now` 统一写 | `src/settings/form.rs::flush_and_save_now` | **PASS** |
| 6.2.7 修改路径不影响已运行 WT | `init_delayed_check` 调 `ConfigService::load_or_default` | `src/services/wt.rs:51-56` | **PASS** |
| 6.2.8 「打开配置文件夹」按钮改名 | 「打开配置目录」保留 + 独立命名 | `ui/settings.slint:683` 按钮 `text: "📁 打开配置目录"`；新按钮 `text: "打开"`（卡片 3） | **PARTIAL**（新按钮太简化，"打开数据目录"被缩成"打开"；req §6.2.8 没有强约束按钮文案，但"打开 go-musicfox 数据目录" → "打开"对用户认知略弱） |
| 6.2.9 按钮右侧显示当前解析目录 | `musicfox-data-dir-hint` | `ui/settings.slint:504-511` `text: root.musicfox-data-dir-hint` + `src/path.rs:56-62` `musicfox_data_dir_display()` | **PASS** |
| 6.3 DataDir 4 级 fallback | MUSICFOX_ROOT → XDG_DATA_HOME → LOCALAPPDATA → USERPROFILE | `src/path.rs:18-53` | **PASS** |
| 6.4 字段落盘示例 | `[wt]` 节三项 | `tests/config_test.rs::test_wt_config_defaults_and_roundtrip` 验证 | **PASS** |
| 6.5 验收 12 项 | — | — | 见 `review-finding.md` §3.4 |

## §7 设置界面视觉现代化

| 验收项 | 期望 | 代码位置 | 状态 |
|---|---|---|---|
| 7.2.1 浅灰底 + 纯白卡片 + 圆角 + 1px 边框 | `background: #f8f9fa` + `Rectangle { background: #ffffff; border-radius: 8px; border-width: 1px; border-color: #e0e2ec; }` | `ui/settings.slint:79 background: #f8f9fa` + 各卡片 `background: #ffffff; border-radius: 8px; border-width: 1px; border-color: #e0e2ec` | **PASS** |
| 7.2.2 卡片容器 4 个 | 🪟 / 🎨 / ⚙️ / 🎵 | `ui/settings.slint:165 / 209 / 459 / 528`（顺序：窗口 / 样式 / go-musicfox / 系统 — 见 §6.2.1 反了） | **PARTIAL**（卡片齐全但**顺序错误**） |
| 7.2.3 表单排版与对齐 | 标签宽度 90px | `ui/settings.slint:171, 220, 469, 533` `width: 80px`（不是 90px） | **PARTIAL**（dev.md §3.1 写"标签列宽度：固定 90px 严格左对齐"，实际 80px） |
| 7.2.4 底部操作栏美化 | 主按钮高亮 | `ui/settings.slint:684-687` `Button { text: "💾 保存"; primary: true; }` | **PASS** |
| 7.2.5 模态对话框美化 | 半透明遮罩 + 圆角卡片 | `ui/settings.slint:746, 786` `background: #66000000` + `border-radius: 12px` | **PASS** |
| 7.2.6 Toast 浮动通知 | 右下角深色圆角矩形 | `ui/settings.slint:725-740` `background: #313033f2; border-radius: 6px` | **PASS** |
| 7.3 验收 | — | — | — |

## §8 设置界面全量功能 F01~F14

| 编号 | 测试场景 | 实现 / 测试覆盖 | 状态 |
|---|---|---|---|
| F01 | 正常配置加载 | `tests/settings_test.rs::bootstrap_prefers_valid_tmp_over_config` 等；`form.bootstrap()` 完整 | **PASS** |
| F02 | 首次运行默认创建 | `tests/settings_test.rs::bootstrap_creates_default_when_missing` | **PASS** |
| F03 | 临时副本恢复 | `tests/settings_test.rs::bootstrap_prefers_valid_tmp_over_config` + `bootstrap_discards_invalid_tmp_and_recovers_from_config` | **PASS** |
| F04 | 损坏恢复 | `tests/settings_test.rs::broken_config_triggers_parse_modal_and_resets_with_backup` | **PASS** |
| F05 | 窗口实时位置拉取 | `src/settings/mod.rs::sync_to_ui` + `spawn_pos_query` + 一次性 pos 应用 | **PASS** |
| F06 | 位置离线降级 | `src/settings/mod.rs:560-568` 通道 `Disconnected / Stale` → `set_pos_stale(true)` | **PASS** |
| F07 | 表单修改与防抖暂存 | `tests/settings_test.rs::debounce_500ms_writes_tmp_but_does_not_clear_dirty` + `form::flush_tmp_if_due` | **PASS** |
| F08 | 字段合法性校验拦截 | `tests/settings_test.rs::invalid_save_keeps_formal_config_intact` + `validate_all` | **PASS** |
| F09 | 保存并实时热重载 | `form::flush_and_save_now` 原子写 OK；但**通知方式**为 `restart_lyric_app()` 而非 IPC reload → 与 dev.md §P0 step 2 设计不符 | **PARTIAL** |
| F10 | 重载歌词独立反馈 | **无 UI 按钮**，无差异化 toast | **FAIL** |
| F11 | 系统字体族枚举 | `src/services/font.rs::system_families()` + `ui/settings.slint` ComboBox | **PASS**（未跑测试，但代码完整） |
| F12 | 打开配置目录 | `src/settings/mod.rs::open_config_folder` | **PASS** |
| F13 | 未保存修改关闭拦截 | `src/settings/mod.rs:494-507` `on_close_requested` 弹模态 + save/discard/cancel | **PASS** |
| F14 | 单实例与激活 | `src/settings/mod.rs::acquire_settings_mutex` + `pipe::presence::try_activate_existing` | **PASS** |

> F09 与 F10 共同构成"设置进程与主进程 IPC 联动"的核心用户故事；当前实现退化为"杀旧拉新"。这在视觉/易用性上是简化，但偏离了 req §2 的明确语义，应在评审中标记。

## 附加：P0-P3-bug.md 行动项落实

| 项 | 期望 | 实际 | 状态 |
|---|---|---|---|
| 移除 `with_menu` | 让左键不再自动弹菜单 | `src/platform/windows/mod.rs:135` `TrayIconBuilder::new().with_tooltip(...).with_icon(...).build()` **未调用 with_menu** | **PASS** |
| 区分左键 = ToggleWt | — | `src/platform/windows/mod.rs:163-167` `MouseButton::Left → TrayCmd::ToggleWt` | **PASS** |
| 移除右键菜单的「go-musicfox」项 | 菜单 = 【配置 / 重载 / 退出】 | `src/platform/windows/mod.rs:218-225` 菜单 = 配置 / 重载 / 退出 | **PASS** |
| 紧凑输入框 32px | height: 32 | `ui/settings.slint:36` `height: 30px`（30 vs 32 差 2px；"紧凑"语义满足） | **PARTIAL** |
| 移除 ScrollView | 100% 无滚动条 | `ui/settings.slint:130` **ScrollView 仍在** | **FAIL** |
| 端口纯文本 + 删除说明 | 端口改 `Text` + 删除下方灰色说明 | `ui/settings.slint:544` 是 Text；灰色说明**已删**（dev.md §P1 Step 3 要求保留；req §3.2.4 也要求保留） | **PARTIAL** |
| 预设调色板 + 当前颜色预览 | 7 色块 + 当前色块 | `ui/settings.slint:271-279` 8 个 `ColorPill` + `preview-text-color` 色块 | **PASS** |

> BUG-01 / BUG-02 / BUG-03 的根因已切到正确的修复路径（移除 with_menu）；**根因 #1 的"Y 锚点边界保护"未完整实现**，见 `review-finding.md` §3.2。