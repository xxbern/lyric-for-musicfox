# 方案① 落地计划：歌词悬浮窗迁移 Direct2D + DirectWrite 原生渲染

日期：2026-08-25
状态：规划已确认，待指令实施
前置：P1–P5 已完成；内存归因见 `memory_report_20260825.md` 第五轮（eframe/OpenGL 驱动 +82MB、图集+首帧 +44MB、字体文件 20MB）。

---

## 0. 已确认决策

| 决策点 | 结论 |
|---|---|
| D1 歌词窗口 Windows-only | ✅ 接受（Linux 仅保留协议层测试，现状即如此） |
| D2 settings 的 egui 依赖 | ✅ **方案 B：共存**。单 exe 同时包含 egui（设置窗口）与 D2D 路径（歌词窗口），歌词运行路径不触碰 egui；方案 A（拆分独立 settings exe，省 ~3MB + 二进制瘦身）无限期搁置，必要时作为独立后续任务 |
| D3 文字描边实现 | ✅ 8 向偏移双 pass 近似（简单够用） |

里程碑 M5 相应简化：移除"方案 B→A 收尾"，仅保留内存对比报告与验收矩阵修订。

---

## 0. 目标与预期收益

| 指标 | 现状（egui/glow） | 方案① 预期 |
|---|---|---|
| 进程 commit | ~148MB | **~25–40MB** |
| GL 驱动上下文 | +82MB | 0（无 GL，D2D/DWrite 走系统共享运行时） |
| 字体字节常驻 | +20MB | 0（DWrite 直接用系统字体集合，删除字节加载管线） |
| 图集纹理 | +44MB | 0（D2D 直接光栅化，无图集） |
| 静止时 CPU | 低（repaint timer） | 0（纯事件驱动，滚动时才挂 60fps 定时器） |

附带收益：ClearType 亚像素渲染质量优于 egui；VF 变量字体原生支持。

代价：**歌词窗口不再跨平台**（Linux 下走 stub，仅保留协议层测试——与现有验收矩阵一致）。

## 1. 总体架构

```
main.rs
 └─ window::run()                       [重写] 不再调用 eframe::run_native
     ├─ 注册 WNDCLASS → CreateWindowExW（WS_POPUP | WS_EX_LAYERED |
     │    WS_EX_TRANSPARENT(锁定时) | WS_EX_TOOLWINDOW | WS_EX_TOPMOST）
     ├─ 渲染循环：消息泵线程 = 主线程
     │    - WM_TIMER(16ms) 或 WM_PAINT → render_frame()
     │    - UpdateLayeredWindowIndirect 提交到屏幕
     ├─ 后台线程：EventBus 排空 / UDP / 托盘 / pipes（不变）
     │    → 通过 EventBus::RequestRepaint 投递 WM_PAINT（PostMessage）
     └─ services::ServiceHandles 全部复用
```

**保持不变**（这是改动量可控的关键）：
- `AppContext` / `EventBus` / 有界事件总线（P1）
- PAL trait 层与 `platform::current()` 分发（P2）
- `services/*` 七个业务服务（P3），仅 font 服务改签名
- 托盘、pipes、UDP、配置热重载全链路
- **设置窗口进程整体不动**（独立 egui 进程）

## 2. 模块级改动清单

### 2.1 新建

| 文件 | 内容 | 预估行数 |
|---|---|---|
| `src/platform/windows/render_d2d.rs` | D2D 工厂/设备上下文封装、内存位图、`UpdateLayeredWindowIndirect` 提交 | ~250 |
| `src/platform/windows/text_dwrite.rs` | DWrite 工厂、TextFormat 缓存、TextLayout 构建（描边双 pass）、颜色/alpha 应用 | ~200 |
| `src/window/native.rs` | Win32 窗口生命周期：类注册、创建、消息泵、WM_TIMER 驱动、`PostMessage(WM_APP_REPAINT)` 桥接 | ~300 |
| `src/window/hit_test.rs` | 基于 alpha 位图的拖拽热区判定（替代现 egui pointer 输入） | ~80 |

### 2.2 重写

| 文件 | 改动 | 预估行数 |
|---|---|---|
| `src/window/mod.rs` | 删除 `impl eframe::App` 与 `native_options`；`run()` 改为构建原生窗口 + 消息泵；事件排空逻辑平移到 `WM_APP_*` 处理器 | 473→~350 |
| `src/window/render_cache.rs` | galley 缓存 → `TextLayout` 缓存（失效条件不变，接口几乎同名） | 105→~90 |
| `src/services/font.rs` | **大幅简化**：删除注册表定位/文件读取/GDI 枚举；`resolve_family` → DWrite `GetSystemFontCollection.FindFamilyName`；`apply_to_egui` 删除。惰性 `LazyFontList` 改为枚举 DWrite 集合（仍可保留惰性语义） | 314→~150 |

### 2.3 微调（逻辑原样保留）

| 文件 | 改动 |
|---|---|
| `src/window/scroll.rs` | 零改动（纯数学，输入 text_width/offset 即可） |
| `src/window/drag.rs` | 零改动（输入物理坐标即可；指针来源从 egui input 换成 `WM_LBUTTONDOWN/WM_MOUSEMOVE`） |
| `src/tray`、`pipe`、`lyric/udp`、`context`、`event_bus` | 零改动 |
| `src/main.rs` | `--settings` 分支不变；主分支调用新 `window::run()` |
| `Cargo.toml` | 主 crate 移除 eframe/egui 依赖；新增 windows features：`Win32_Graphics_Direct2D`、`Win32_Graphics_DirectWrite`、`Win32_Graphics_Imaging`(D2D 需)、`Win32_UI_Input_KeyboardAndMouse` 等。设置子进程需要 eframe → 拆分为 feature `settings-ui` 或将 settings 移至独立 bin 共享 lib |

### 2.4 删除

- `src/window/render.rs` 中 egui Galley 相关（`layout_lyric`/`paint_lyric` 的 egui 实现移入 DWrite 模块）；`parse_rrggbb`/`color_with_alpha` 保留
- `eframe`/`egui` 在主窗口路径的全部引用（stub 平台后端同步瘦身）

### 2.5 设置窗口依赖处理（已决策：方案 B）

eframe/egui 保留为常规依赖，settings 代码不动；歌词窗口运行路径不初始化 egui/GL。
主进程内存收益（省 82MB GL 驱动）不受影响，仅二进制中存在未被歌词路径执行的 egui 代码。
方案 A（拆分独立 exe / feature 门控，省 ~3MB）记录为可选后续任务，暂不排期。

CI 注意：无需双编译矩阵；现有 `cargo test` 与 Windows 交叉编译检查照旧。

## 3. 关键技术点与风险

| # | 点 | 说明 | 风险 |
|---|---|---|---|
| R1 | 透明合成 | 分层窗口要求预乘 alpha BGRA 位图；D2D `DCRenderTarget` 天然输出预乘 alpha | 低，标准做法 |
| R2 | 点击穿透切换 | 锁定=WS_EX_TRANSPARENT；解锁后需 hit-test：读取离屏位图 alpha，透明区放行点击 | 中，参考旧版实现（历史上有 hit-test_applied 逻辑可借鉴） |
| R3 | 拖拽 | 解锁态 `WM_NCHITTEST` 返回 HTCAPTION 或手动处理 WM_LBUTTONDOWN+SetWindowPos | 低 |
| R4 | 高 DPI | D2D/DWrite 原生支持 per-monitor DPI；需处理 `WM_DPICHANGED` | 中，比 egui 更可控 |
| R5 | 多显示器裁剪 | 复用 `MonitorService`（纯几何，零改动） | 低 |
| R6 | VF 变量字体 | DWrite3 `CreateFontFaceReference` 支持 axis；若嫌复杂可先按默认实例渲染（视觉与 egui 版一致） | 低 |
| R7 | 描边（outline）效果 | 现有 outline_width 参数：DWrite 无直接 stroke API，需 ID2D1Geometry realizations 或双 TextLayout 偏移近似（8 向偏移法，性能足够） | 中 |
| R8 | 滚动平滑 | 滚动改为每帧 offset += speed*dt，`InvalidateRect` 局部刷新；静止时 kill timer | 低 |
| R9 | windows crate features 体积 | D2D/DWrite features 编译期声明，运行时共享系统 DLL，不增加常驻 | 低 |
| R10 | 回退方案 | 保留现 egui 路径为 `window_legacy`（feature 门控），一个发布周期后再删 | 低 |

## 4. 里程碑拆分（对应 subagent 任务粒度）

| 阶段 | 内容 | 完成判据 | 预估 |
|---|---|---|---|
| M1 | native.rs 窗口骨架 + 消息泵 + 空 D2D 清屏分层窗口上屏 | Windows 上出现透明置顶空窗，commit ≤40MB（mem_diag 打点验证） | 1 天 |
| M2 | text_dwrite + render_cache：单行静态文本 + 颜色/alpha + 8 向偏移描边 | 显示"......"占位文本，样式与 egui 版肉眼一致 | 1–1.5 天 |
| M3 | scroll + timer + 事件桥接（RequestRepaint→PostMessage、UDP/托盘/pipes 联调） | go-musicfox 推流滚动显示正常，锁定穿透生效 | 1 天 |
| M4 | drag/hit_test + MonitorService 裁剪接入 + on_exit 位置落盘 | 拖动、副屏裁剪、退出保存全部回归通过 | 1 天 |
| M5 | mem_diag 对比报告、验收矩阵修订（方案 B，无拆分工作） | CSV：commit ≤40MB；cargo test 全绿；Win 交叉编译产物正常 | 0.5 天 |

总量：**4.5–5 人日**（其中 M1/M2 是主要不确定性来源：D2D 设备上下文与分层窗口的组合调试）。

## 6. 验证计划

- 复用 `mem_diag.csv` 打点体系，新增标签：`d2d_created`、`first_frame_done`
- 功能回归清单：滚动/暂停/占位符、锁定穿透开关、拖拽、副屏越界收拢、置顶保活、托盘三命令、RELOAD/pos/presence 三管道、配置热重载
- 自动化：现有 18 个测试目标中除 `scroll_test`/`drag_test`/`monitor_test` 外均不受影响；这三个本就是纯逻辑测试，应零改动通过
- 内存验收：冷启动 5s、连续保存配置 10 次、稳态 10 分钟三段采样，commit 目标 ≤40MB 且曲线平稳

## 6. 决策请求

1. 是否接受「歌词窗口 Windows-only」？（Linux 仅保留协议层测试，现状即如此）
2. settings 依赖选 B（先快速落地）还是直接 A？
3. 描边实现选 R7 双 pass 近似（简单够用）还是几何 stroke（更精致、工作量大）？

确认后按 M1→M5 逐阶段派发执行。
