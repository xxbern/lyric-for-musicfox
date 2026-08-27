# 方案③ 评估：迁移 Slint

日期：2026-08-25（修订 2026-08-26）
状态：评估稿，待决策
关联：`d2d_render_migration_plan.md`（方案①）、`dioxus_migration_plan.md`（方案②）

---

## 0. 背景（历史评估结论摘要）

五轮内存排查（详见 `memory_report_20260825.md`）已收敛为单一根因：
**egui/glow 的 OpenGL 驱动上下文 +82MB、字形图集+首帧惰性分配 +44MB、字体文件本体 20MB**。
字体侧经 P4-R 定向查询改造后已最优（<4MB），剩余 ~126MB 均为 GPU 渲染栈固有成本，
任何 egui 路线都无法达标。因此需要换渲染栈：方案① D2D 原生、方案② Dioxus、本方案③ Slint。

## 1. Slint 关键事实（官方文档/仓库核实）

- 成熟度：**生产级**（工业嵌入式广泛使用），声明式 `.slint` 语言 + Rust 绑定，Live-Preview 工具链
- 窗口能力原生内建：`no-frame`、`background: transparent`、**`always-on-top` 属性**
- 渲染后端可选；关键路线是 **skia 软件光栅**：
  - 官方 issue #6072 维护者确认 `renderer-skia` + `SLINT_BACKEND=winit-skia-software`
    避免驱动双缓冲常驻，"内存占用非常小"
  - 社区实测 Windows 桌面宠物类应用（与本应用形态几乎相同）**~23MB 内存**
- 许可证：GPLv3 / 商业授权 / **Royalty-Free 桌面许可**（桌面应用免费闭源可用，需确认合规接受度）

## 2. 与本应用需求逐项对照

| 需求 | Slint 表现 |
|---|---|
| 全透明无边框置顶 | ✅ `no-frame` + `transparent` + `always-on-top` 原生属性 |
| 动态鼠标穿透 | ⚠️ 框架不直接提供；仍需 raw-window-handle 取 HWND 切 WS_EX_TRANSPARENT（复用现有 PAL 实现） |
| 中文/CJK 渲染 | ✅ skia 后端接系统字体管理（Windows 走 DirectWrite），按需加载——现有定向查询保留，无字节常驻 |
| 描边（outline_width） | ⚠️ 无内建 stroke；与方案①相同用 8 向偏移多 Text 元素近似（声明式写法更简单） |
| 内存 | ✅ 预期 commit **20–40MB**（软件光栅，无 GL/wgpu 驱动上下文），待实测 |
| 设置窗口 | ✅ 内建 LineEdit/ComboBox/Button 等 widget，可与歌词窗统一框架，**彻底移除 egui/eframe** |

## 3. 三方案终局对比

| | 方案① D2D/DWrite | 方案② Dioxus | 方案③ Slint(skia-sw) |
|---|---|---|---|
| 歌词进程 commit | ~25–40MB | 150MB+/200MB+ | **预期 20–40MB（待实测）** |
| 成熟度 | 系统 API，低风险 | alpha/webview | 生产级 |
| 开发体验 | 手写渲染，最底层 | RSX+热重载最好 | 声明式 .slint + Live-Preview |
| 设置窗口 | 保留 egui（共存 B） | 可迁但 webview 笨重 | **可统一迁入，删掉 egui** |
| 跨平台 | 歌词窗 Windows-only | 跨平台 | 跨平台（软件光栅全平台一致） |
| 许可 | 无约束 | MIT/Apache | GPLv3 或 Royalty-Free 桌面许可（需确认合规） |
| 工作量 | 4.5–5 人日 | 5–6+ 人日 | **核心 3–3.5 人日**（见 §4） |

## 4. 迁移成本完整评估

### 4.1 改动面盘点（基于当前代码实际行数）

| 类别 | 文件 | 处理 |
|---|---|---|
| 重写 | `window/mod.rs` (473 行) | 删除 `impl eframe::App`，改为 Slint 组件宿主 + 事件桥接 → ~300 行 |
| 重写 | `window/render.rs` + `render_cache.rs` (205 行) | egui Galley → .slint Text 元素属性绑定 → ~120 行 |
| 大幅简化 | `services/font.rs` (361 行) | 字节加载管线删除，仅保留 GDI 存在性检查供设置窗下拉框 → ~150 行 |
| 零改动 | `scroll.rs`(88)、`drag.rs`(108) | 纯数学/几何，输入物理坐标即可 |
| 零改动 | EventBus / UDP / 托盘 / pipes / MonitorService / 配置热重载 | 全部原样保留 |
| 新增 | `.slint` 歌词组件 + main 接入 | ~200 行 |
| 删除 | Cargo.toml 中 eframe/egui 依赖 | 二进制瘦身；设置窗迁移后才可彻底删除 |

### 4.2 工作量拆分

| 项 | 内容 | 预估 |
|---|---|---|
| S1 | 歌词窗 `.slint` 骨架（透明置顶无边框）+ skia 软件光栅接入 + mem_diag 打点验证 | 1 天 |
| S2 | 单行文本 + 8 向偏移描边 + 颜色/alpha + RenderCache→Slint 属性绑定 | 0.5–1 天 |
| S3 | 滚动（timer 属性驱动）+ EventBus/UDP/托盘/pipes 桥接 + 穿透切换（HWND 复用 PAL 代码） | 1 天 |
| S4 | 拖拽 + MonitorService 裁剪 + 回归清单过一遍 | 0.5 天 |
| S5 | （可选延伸）设置窗口迁 Slint widgets、移除 egui | +1–2 天 |
| 合计 | 核心歌词窗 **3–3.5 人日**；含设置迁移 4.5–5.5 人日 | |

对比：比方案① 少约 1 人日（省去手写 Win32 窗口/D2D 设备上下文调试），且 S5 完成后可整体移除 egui。

### 4.3 技术风险

| # | 风险 | 缓解 |
|---|---|---|
| R1 | skia C++ 构建在 Windows gnu toolchain 交叉编译下的复杂度（对工具链敏感）；若受阻退回 femtovg(GL) 会重新引入驱动开销 | S1 首日即验证构建链，受阻立即止损退回方案① |
| R2 | `winit-skia-software` 分层窗口透明合成质量（预乘 alpha 正确性）需真机验证 | S1 打样中用纯色/渐变测试用例覆盖 |
| R3 | 字体走 Skia/DirectWrite 后 family 匹配语义变化（MiSans VF 等），需回归字体回退逻辑 | 保留现有 resolve 校验层做回归对照 |
| R4 | Royalty-Free 许可合规条款需人工确认 | 决策前确认，不阻塞打样 |

## 5. 决策请求

- [ ] A. 改用方案③ Slint（推荐先做 S1 打样：1 天出内存实测数据再决定是否继续）
- [ ] B. 维持方案① D2D（确定性最高）
- [ ] C. 其他组合

建议路径：**S1 打样 → 看 CSV 数据 → 达标则继续 S2–S4，不达标退回方案①**。打样成本仅 1 天，信息价值最高。
