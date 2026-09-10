# 歌词排版位移与状态机自愈方案

> **分支**：`fix/lyric-drift`  
> **状态**：已完成 (Completed)  
> **关联议题**：播放中歌词频繁发生位移/偏位，右键托盘重载歌词后恢复正常；前两版针对窗口外框的迭代未能解决。

---

## 一、 问题背景与根因定位

在长期运行与歌曲播放过程中，桌面歌词文本频繁出现位移、偏左截断或滚动起点突变的现象；而右键托盘点击【重载歌词】后，歌词能够立即居中并恢复正常。

### 核心根因：
1. **主循环度量与下发时序颠倒**：
   - 在 `src/window/mod.rs` 的每一帧 `tick` 中，代码在未将新文本下发给 Slint 前便调用 `app.get_lyric_text_width()`，读取的是上一句旧歌词的度量宽度。
   - 当短歌词切换为超长歌词时，状态机以旧短文本宽度计算判定为无需滚动（`offset_x = 0.0`），而 Slint 渲染长文本后激活滚动分支（`base-x = offset-x = 0.0`），导致长文本直接顶在窗口左侧边缘（x=0）并开始向左滚动。
2. **UDP 模块未触发重算事件**：
   - `src/lyric/udp.rs` 接收并写入新歌词后，未向 `EventBus` 发送 `AppEvent::LyricStateChanged`，导致主循环未能获知歌词状态跃迁以触发两阶段延迟重算。
3. **Slint 测量元素样式失真**：
   - `ui/lyric.slint` 的测量元素 `measure` 遗漏了 `font-weight <=> root.font-weight` 绑定，加粗文本（font_bold = true）的测量宽度失真偏小。
4. **冷启动与热重载 DPI 倍率不一致**：
   - `src/window/mod.rs` 启动期使用裸配置尺寸传给 `PhysicalSize`，而重载期经过 `scale_factor` 放大，导致不同阶段窗口物理尺寸与逻辑基准漂移。

---

## 二、 修复方案

1. **Slint 测量元素属性对齐**：
   - `ui/lyric.slint` 中为 `measure` 元素补全 `font-weight <=> root.font-weight;` 绑定，保证度量与渲染 100% 样式一致。
2. **UDP 状态更新事件广播**：
   - 在 `src/lyric/udp.rs` 成功解析并应用新歌词后，调用 `ctx.event_bus.emit(AppEvent::LyricStateChanged)`。
3. **两阶段度量与时序纠偏**：
   - 在 `src/window/mod.rs` 中调整主循环帧时序，文本发生变化时先更新 Slint 属性并标记延迟度量，在 Slint 布局稳定帧获取真实自然宽度并正确初始化跑马灯起点。
4. **统一窗口尺寸 DPI 换算**：
   - 启动期与热重载统一按 `scale_factor` 换算逻辑尺寸到 `PhysicalSize`，保持生命周期各阶段视口逻辑宽度恒定。
5. **自动化测试**：
   - 增加单元测试与集成测试，覆盖新文本切换后的度量与滚动偏移初始化流转。

---

## 三、 执行清单

- [x] **Task 1**：在 `ui/lyric.slint` 为 `measure` 补齐 `font-weight` 绑定。
- [x] **Task 2**：在 `src/lyric/udp.rs` 增加 `LyricStateChanged` 事件通知。
- [x] **Task 3**：在 `src/window/mod.rs` 统一冷启动窗口物理尺寸转换。
- [x] **Task 4**：重构 `src/window/mod.rs` 与 `src/window/scroll.rs` 时序，确保两阶段精准度量。
- [x] **Task 5**：运行全量 `cargo test` 验证修复完整性。
