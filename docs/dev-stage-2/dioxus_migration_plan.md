# 方案② 落地计划（评估稿）：迁移 Dioxus

日期：2026-08-25
状态：**评估中，未批准实施**
关联：`d2d_render_migration_plan.md`（方案①）、`memory_report_20260825.md`

---

## 1. Dioxus 是什么、桌面端有哪两种形态

Dioxus（dioxuslabs/dioxus，当前 0.7.x）是类 React 的 Rust UI 框架（RSX DSL + VirtualDOM）。
桌面渲染有两种互斥形态：

| 形态 | 底层 | 成熟度 |
|---|---|---|
| **desktop**（默认） | wry/tao → 系统 WebView（Windows 上是 Edge WebView2） | 生产可用 |
| **native**（0.7 新增） | Blitz 引擎（Stylo/Taffy/vello）→ **wgpu** | 官方自述 alpha；官方对比文档承认"wgpu renderer 相当不成熟，尚未达到生产可用" |

## 2. 与本应用核心需求的逐项对撞

歌词悬浮窗的四个刚性需求：

| 需求 | desktop(WebView2) | native(wgpu/Blitz) |
|---|---|---|
| 全透明无边框窗口 | wry 支持 transparent，但 WebView 合成透明层有历史 bug | winit 支持 `with_transparent` |
| 动态鼠标穿透（锁定开关） | 需拿到 HWND 手动切 WS_EX_TRANSPARENT；WebView 会吞 WM_NCHITTEST，**热区级穿透基本不可行**（整窗穿透可以） | 同样要绕到 winit 原生句柄手写，框架不提供 |
| 内存目标 ≤40MB | ❌ **WebView2 进程组典型 100~250MB**（Chromium 内核），比现状 egui/glow 更差 | ❌ wgpu 栈与 glow 同量级（驱动上下文换壳），叠加 Stylo/Taffy/vello 运行时，预期 ≥ 现状 |
| 单行文字描边滚动 | CSS 可做但经 JS 桥，滚动动画走 web 合成器 | vello 文本渲染可做，alpha 通道成熟度未知 |

结论先行：**两种形态都无法满足内存验收线**；
- WebView2 路线会让内存指标**恶化**（148MB → 200MB+）
- Native 路线内存不会优于现状，且引入一个官方标注 alpha 的渲染引擎来替换一块已经验证可行的功能面

## 3. 若仍采用 Dioxus，推荐架构

若团队价值在于「RSX 声明式 UI + 热重载开发体验」，合理的落位是：

```
lyric-for-musicfox.exe
├─ 歌词悬浮窗：保留 D2D/DWrite（方案①）或 egui（现状）
│    —— 不交给 Dioxus，理由见 §2
└─ 设置窗口：dioxus desktop (WebView2)
     ← 表单类 UI，WebView2 内存开销在设置进程内可接受（按需启动、用完退出）
```

- 设置进程只在用户点"配置"时启动，关闭即释放；主进程常驻内存不受影响
- RSX 写表单效率高于 egui 手排布局；Tailwind/热重载提升迭代速度
- 主进程 ↔ 设置进程已有 presence/reload 管道通信，天然适配双进程

## 4. 工作量评估（§3 架构：仅设置窗口迁 Dioxus）

| 项 | 预估 |
|---|---|
| workspace 拆分（lib 核心共享 + settings-dioxus bin） | 1 天 |
| 表单迁移（字段绑定、校验接入 validate.rs、保存走 ConfigService） | 2–3 天 |
| 托盘/presence 启动链路适配新 exe | 0.5 天 |
| WebView2 运行时分发检查（Win10 老版本需装 Evergreen Runtime） | 0.5 天 |
| 回归测试 | 1 天 |
| **合计** | **5–6 人日** |

歌词悬浮窗本身若也要 Dioxus（不推荐）：+5–8 人日，且穿透/透明细节风险高。

## 5. 三方案终局对比

| | 现状 egui/glow | 方案① D2D（已确认待实施） | 方案② Dioxus |
|---|---|---|---|
| 歌词进程 commit | ~148MB | **~25–40MB** | ~150MB+（native）/ ~200MB+（webview） |
| 点击穿透/拖拽 | 已实现可用 | 直接 Win32，最可控 | 绕过框架手写 HWND |
| 开发体验 | 即时模式一般 | 手写渲染较底层 | RSX + 热重载最好 |
| 成熟度风险 | 低 | 低（系统 API） | 高（Blitz alpha / WebView2 版本碎片） |
| 工作量 | 0 | 4.5–5 人日 | 5–6 人日（仅设置窗）/ 更多（含歌词窗） |

## 6. 建议

1. **歌词悬浮窗维持方案①（D2D）**——它是内存问题的唯一有效解
2. **设置窗口可选迁 Dioxus desktop**，动机应为开发体验而非内存；作为独立任务排期
3. 若目标是"全面 Dioxus"，需要先接受：内存指标作废或放宽至 ~200MB，且依赖 alpha 渲染器

## 7. 决策请求

- [ ] A. 维持方案①（D2D），Dioxus 仅用于设置窗口后续演进
- [ ] B. 全量 Dioxus：接受内存恶化 + alpha 风险，重写全部 UI
- [ ] C. 其他组合（如：歌词窗 D2D + 设置窗暂留 egui 观望）
