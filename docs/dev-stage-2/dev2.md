# 开发设计与实施手册 (dev2.md)

> 版本：0.3.0（下一阶段实现设计）
> 适用对象：开发人员（Dever）、评审人员与测试人员
> 范围：P0 ~ P3 阶段实现细节、代码级规范、执行步骤与独立验收指南
> 状态：设计定稿，待代码库解除只读后实施

---

## 1. 设计原则与整体优化指标

1. **极致轻量与节能（Performance First）**：桌面歌词挂件属于常驻轻量插件，空闲静止状态 CPU 占用必须趋近 **0.0%**（完全休眠，无忙轮询）；常驻内存从目前的 **~277MB** 彻底压降至 **< 30~50MB**。
2. **原生桌面 GUI 规范（Native GUI Experience）**：彻底消除 Windows 双击启动时的控制台黑框；歌词悬浮窗口脱离 Windows 任务栏（不占栏位），仅作为桌面悬浮层；设置窗口保持标准应用行为，正常在任务栏显示并支持前台激活。
3. **极简高效交互（One-Click Save & Apply）**：配置修改后一键【保存并应用】，通过现有的 Windows Named Pipe 管道实现主进程即时热重载，零多余确认弹窗阻断；右键托盘与设置界面底部均提供显式【重载歌词】入口。
4. **视觉现代化（Modern Minimalist）**：设置界面重构为单页垂直滚动 + 扁平卡片（Grouped Cards）布局，统一圆角、间距与卡片层级，提升交互反馈。
5. **向后兼容与平滑迁移（Backward Compatibility）**：保持旧版 `config.toml` 的完全兼容，反序列化自动补齐 `log_enabled` 默认值，序列化时静默忽略废弃的 `frame_less` 配置项。

---

## 2. 阶段总览 (P0 ~ P3)

```
┌────────────────────────────────────────────────────────────────────────┐
│ P0: 基础窗口规范与配置升级 (Win32 Subsystem / Taskbar / Config / Logger) │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
┌──────────────────────────────────▼─────────────────────────────────────┐
│ P1: 渲染与资源深度优化 (CPU 0% 按需重绘 / 内存轻量化 / FontDB 释放)      │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
┌──────────────────────────────────▼─────────────────────────────────────┐
│ P2: 歌词热重载全链路打通 (托盘重载 / IPC 命名管道 / 保存即时热重载)       │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
┌──────────────────────────────────▼─────────────────────────────────────┐
│ P3: 设置界面现代单页卡片化重构 (Modern Card UI / 扁平视觉 / 交互升级)    │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. P0 ~ P3 模块化开发说明书

---

### 🟢 P0 阶段：基础窗口规范与配置系统升级

#### 1. 阶段目标
- 双击 Release 可执行文件启动时**无控制台黑框弹出或闪烁**（Debug 调试模式保留控制台输出）。
- 歌词悬浮窗口**不在 Windows 任务栏显示**（不占任务栏空间）；设置窗口在任务栏**正常显示**。
- 歌词窗口固定为**无边框**，配置文件移出 `frame_less` 项并保证旧配置向后兼容。
- 配置文件新增 `log_enabled`（默认 `false`），关闭时不产生日志文件、不启动后台写盘线程。

#### 2. 涉及文件清单
- `src/main.rs`
- `src/window/hit_test.rs`
- `src/config/mod.rs`
- `src/logger.rs`

#### 3. 具体实现步骤与代码规范

##### Step 1: `src/main.rs` 声明 Windows GUI 子系统
在 `src/main.rs` 文件最顶部第 1 行添加属性宏：
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
```
- **机制**：Release 模式使用 `windows` 子系统，操作系统不创建 Console 窗口；Debug 模式保留 `console` 子系统输出 `eprintln!`。

##### Step 2: `src/window/hit_test.rs` 注入 `WS_EX_TOOLWINDOW`
在获取到主窗口 `HWND` 句柄时，统一管理扩展样式（`GWL_EXSTYLE`）：
```rust
#[cfg(windows)]
pub fn apply_taskbar_and_locked_style(hwnd: HWND, locked: bool) {
    unsafe {
        let prev = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        // 关键：注入 TOOLWINDOW，剥离 APPWINDOW，根据 locked 切换 TRANSPARENT
        let mut new = (prev | (WS_EX_TOOLWINDOW.0 as isize)) & !(WS_EX_APPWINDOW.0 as isize);
        if locked {
            new |= WS_EX_TRANSPARENT.0 as isize;
        } else {
            new &= !(WS_EX_TRANSPARENT.0 as isize);
        }
        if new != prev {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
            let _ = SetWindowPos(
                hwnd,
                HWND_TOP,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}
```
- **注意**：设置窗口（`SettingsApp`）不调用此函数，保持原有正常窗口样式，正常在任务栏显示。

##### Step 3: `src/config/mod.rs` 调整配置结构
1. `WindowConfig` 中的 `frame_less` 增加 `skip_serializing`：
   ```rust
   #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
   pub struct WindowConfig {
       pub width: u32,
       pub height: u32,
       // ...
       pub stay_on_top: bool,
       #[serde(default = "default_true", skip_serializing)]
       pub frame_less: bool,
       pub locked: bool,
   }
   ```
2. `SystemConfig` 新增 `log_enabled` 字段（默认 `false`）：
   ```rust
   #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
   pub struct SystemConfig {
       #[serde(default = "default_receive_port")]
       pub receive_port: u16,
       #[serde(default = "default_send_port")]
       pub send_port: u16,
       #[serde(default = "default_false")]
       pub log_enabled: bool,
   }
   ```

##### Step 4: `src/logger.rs` 支持开关控制
修改 `logger::init(log_path: &Path, default_level: LevelFilter, enabled: bool) -> LoggerGuard`：
- 若 `enabled == false`：
  - `log::set_max_level(LevelFilter::Off)`；
  - 直接返回 `LoggerGuard { tx: None, thread_handle: None }`，不创建 channel，不启动后台线程；
- 若 `enabled == true`：
  - 按原逻辑初始化通道与文件写入线程。

#### 4. P0 独立验收标准
1. 执行 `cargo build --release`，双击生成在 `target/release/lyric-for-musicfox.exe`，确认**没有任何控制台黑框弹出**。
2. 启动后查看 Windows 任务栏，确认**没有歌词窗口图标**；打开 `--settings` 确认**设置窗口正常出现在任务栏**。
3. 检查默认运行生成的 `config.toml`，确认无 `frame_less` 字段，且 `log_enabled = false`。
4. 默认启动运行 1 分钟，确认 `%APPDATA%/lyric-for-musicfox/` 目录下**不生成 `log.txt`**。

---

### 🟢 P1 阶段：CPU 与内存深度优化 (核心性能压降)

#### 1. 阶段目标
- 歌词静止/无拖动/占位状态下，CPU 占用 **趋近 0.0%**（彻底消除原满帧空转 `request_repaint`）。
- 歌词超出宽度跑马灯滚动时，平滑以约 60FPS（16ms）节流推进，CPU 占用 **< 0.5%**。
- 移除主 App 中常驻的 `fontdb::Database` 几十兆元数据，常驻内存降至 **< 30~50MB**。
- UDP 收到新歌词包时精确唤醒 egui 主循环。

#### 2. 涉及文件清单
- `src/window/mod.rs`
- `src/window/scroll.rs`
- `src/settings/mod.rs`
- `src/lyric/udp.rs`

#### 3. 具体实现步骤与代码规范

##### Step 1: `src/window/mod.rs` 重写重绘调度逻辑
**彻底删除** `LyricApp::update()` 末尾的无条件 `ctx.request_repaint()`。
替换为**状态感知与协同节流调度**：
```rust
// 1. 状态判断
let is_scrolling = ScrollState::should_scroll(text_width, window_width) && playing;
let is_dragging = self.drag.is_active();
let stay_on_top_due = self.config.window.stay_on_top;

// 2. 调度分支
if is_scrolling {
    // 跑马灯滚动中：约 60FPS 节流重绘
    ctx.request_repaint_after(std::time::Duration::from_millis(16));
} else if is_dragging {
    // 鼠标拖动中：实时响应
    ctx.request_repaint();
} else if stay_on_top_due {
    // 静止且开启置顶：以 500ms 间隔轻量调度，保障置顶不丢失同时 CPU 0%
    ctx.request_repaint_after(std::time::Duration::from_millis(500));
} else {
    // 静止无交互：不调用 request_repaint，进程完全休眠！
}
```

##### Step 2: `src/window/mod.rs` 释放常驻 `fontdb` 内存
1. 从 `LyricApp` 结构体中移除 `font_db: fontdb::Database` 字段。
2. 改为在启动/重载时调用临时函数提取字节流，提取完毕后立即 drop：
   ```rust
   pub fn load_font_transient(family: &str, bold: bool, italic: bool) -> (String, Option<Vec<u8>>) {
       let mut db = fontdb::Database::new();
       db.load_system_fonts();
       let resolved = resolve_runtime_font(family, &db);
       let bytes = load_font_bytes(&db, &resolved, bold, italic);
       (resolved, bytes) // db 在此自动释放，省下数十兆系统字体元数据
   }
   ```

##### Step 3: `src/lyric/udp.rs` 实现事件驱动唤醒
当 UDP 线程解析出新的有效歌词或切歌时：
- 设置 `LyricState` 数据；
- 若文本或播放状态发生实质改变，触发重绘通知（可通过共享信号 `signals.needs_repaint` 或传入的 `egui::Context` 调用 `ctx.request_repaint()`）。

#### 4. P1 独立验收标准
1. 启动程序并不播放音乐（显示 `"......"` 占位符），打开 Windows 任务管理器，确认 `lyric-for-musicfox.exe` 的 CPU 占用率持续为 **0.0%**。
2. 播放歌曲且当前行超出宽度开始跑马灯滚动，观察 CPU 占用率稳定在 **< 0.5%**（i5/i7/Ryzen 常见测试机）。
3. 运行 10 分钟后观察专用工作集（Working Set）内存，确认稳定在 **< 30~50MB**（原先为 277MB）。

---

### 🟢 P2 阶段：歌词热重载全链路打通

#### 1. 阶段目标
- 系统托盘右键菜单增加【重载歌词】项，点击后即时刷新配置并重设渲染。
- 设置界面【保存并应用】通过 Windows Named Pipe 管道触发主进程热重载，无需用户重启主进程。
- 跨平台桩（`stub.rs`）保持同步，确保所有测试与平台编译正常。

#### 2. 涉及文件清单
- `src/tray/windows_tray.rs`
- `src/tray/stub.rs`
- `src/tray/mod.rs`
- `src/pipe/windows_pipe.rs`
- `src/window/mod.rs`
- `src/settings/form.rs`

#### 3. 具体实现步骤与代码规范

##### Step 1: 扩展托盘菜单命令
在 `src/tray/windows_tray.rs` 和 `src/tray/stub.rs` 中：
```rust
pub enum TrayCmd {
    OpenSettings,
    ReloadLyrics, // 新增命令
    Quit,
}
```
并在 `windows_tray.rs` 中构建菜单项：
```rust
let config_item = MenuItem::new("配置", true, None);
let reload_item = MenuItem::new("重载歌词", true, None);
let quit_item = MenuItem::new("退出", true, None);
let _ = menu.append(&config_item);
let _ = menu.append(&reload_item);
let _ = menu.append(&quit_item);
```
当点击 `reload_item` 时向 channel 发送 `TrayCmd::ReloadLyrics`。

##### Step 2: 主循环处理重载命令
在 `src/window/mod.rs` 的 `update()` 托盘命令分支中：
```rust
crate::tray::TrayCmd::ReloadLyrics => {
    if let Ok(config_path) = crate::path::config_path() {
        if let Ok(cfg) = crate::load_config(&config_path) {
            self.config = cfg;
            let (resolved, bytes) = load_font_transient(
                &self.config.lyric_style.font_family,
                self.config.lyric_style.font_bold,
                self.config.lyric_style.font_italic,
            );
            self.resolved_font_family = resolved;
            apply_fonts(ctx, &self.resolved_font_family, bytes);
            self.scroll.needs_recompute = true;
            #[cfg(windows)]
            if let Some(hwnd) = self.hwnd {
                hit_test::apply_taskbar_and_locked_style(hwnd, self.config.window.locked);
            }
            ctx.request_repaint();
        }
    }
}
```

##### Step 3: 设置界面保存后发送管道通知
确认 `src/settings/form.rs` 中的 `flush_and_save_now()` 在写入文件后正常调用：
```rust
#[cfg(windows)]
std::thread::spawn(|| {
    crate::pipe::reload::notify_reload();
});
```

#### 4. P2 独立验收标准
1. 启动主程序，修改 `config.toml` 中字体颜色为 `#00FF00`（绿色）。
2. 在托盘图标右键点击【重载歌词】，歌词窗口文字**瞬间变为绿色**。
3. 打开设置界面，修改字号并点击保存，歌词窗口**即刻实时生效**。

---

### 🟢 P3 阶段：设置界面现代单页卡片化重构

#### 1. 阶段目标
- 设置界面采用**现代简约单页卡片（Modern Grouped Cards）**布局，支持垂直滚动。
- 移出“无边框”复选框；增加“启用日志文件输出”复选框。
- 主操作按钮命名为 **【保存并应用】**，保存后右下角弹出 3 秒轻量 Toast：“配置已保存并已实时生效”。
- 底部提供独立的 **【重载歌词】** 次级按钮与 **【打开配置文件夹】** 辅助按钮。

#### 2. 涉及文件清单
- `src/settings/ui.rs`
- `src/settings/form.rs`

#### 3. 具体实现步骤与代码规范

##### Step 1: 封装现代卡片容器 `group_card`
在 `src/settings/ui.rs` 中：
```rust
pub fn group_card<R>(
    ui: &mut egui::Ui,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(title).strong().size(13.5));
    ui.add_space(3.0);
    
    egui::Frame::none()
        .fill(ui.visuals().faint_bg_color)
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
        .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui)
        }).inner
}
```

##### Step 2: 组织三大卡片内容
外层使用 `egui::ScrollArea::vertical().auto_shrink([false; 2])`：

1. **🪟 窗口与定位卡片**：
   - 宽度、高度：并排数字输入框；
   - X 轴、Y 轴：单行输入框（支持留空居中）；
   - 窗口行为：`始终置顶` 与 `鼠标穿透锁定` 开关。
2. **🎨 歌词排版与样式卡片**：
   - 字体族下拉选择框；
   - 字号：Slider + 精确数字调节；
   - 样式修饰：`加粗 (B)` 与 `斜体 (I)` 并排复选框；
   - 字体颜色：色块预览 + 弹出取色器；
   - 描边：`启用描边` 开关、描边颜色、描边宽度。
3. **⚙️ 系统与网络卡片**：
   - UDP 接收端口（16501）、发送端口（16502）；
   - 运行日志：`启用日志文件输出` 复选框（文字标注：*默认关闭以优化系统资源占用*）。

##### Step 3: 底部固定操作栏
```rust
ui.horizontal(|ui| {
    if ui.button(egui::RichText::new("💾 保存并应用").strong()).clicked() {
        self.on_save_and_apply();
    }
    if ui.button("🔄 重载歌词").clicked() {
        #[cfg(windows)]
        crate::pipe::reload::notify_reload();
        self.form.show_toast("已发送重载通知");
    }
    if ui.button("📁 打开配置目录").clicked() {
        open_config_folder();
    }
});
```

#### 4. P3 独立验收标准
1. 打开设置界面，界面呈现清晰的三大卡片式视觉风格，无原先老旧紧密排版。
2. 在设置界面修改字号，点击【保存并应用】，右下角弹出 Toast 提示，且主歌词窗口立即更新。
3. 点击【重载歌词】按钮，弹出“已发送重载通知” Toast，主歌词窗口执行重绘。

---

## 4. 完整验收矩阵表

| 检查阶段 | 验证项 | 测试命令 / 动作 | 预期结果 |
|---|---|---|---|
| **P0** | 控制台黑框 | `cargo run --release` / 双击 exe | 立即出现悬浮窗与托盘，**零控制台黑框闪现** |
| **P0** | 任务栏图标 | 启动主程序与设置程序 | 歌词窗口无任务栏图标；设置窗口有任务栏图标 |
| **P0** | 日志开关 | 检查默认文件生成 | 不生成 `%APPDATA%/lyric-for-musicfox/log.txt` |
| **P1** | 空闲 CPU | 启动不播放音乐，打开任务管理器 | CPU 占用 **0.0% ~ 0.1%** |
| **P1** | 滚动 CPU | 播放长歌词跑马灯滚动 | CPU 占用 **< 0.5%** |
| **P1** | 常驻内存 | 播放歌曲 10 分钟观察 Working Set | 常驻内存稳定在 **< 30~50MB** |
| **P2** | 托盘重载 | 托盘右键点击【重载歌词】 | 重新读取配置并重绘 |
| **P3** | 卡片布局 | 打开设置界面 `--settings` | 现代简约卡片布局，无“无边框”选项 |
| **P3** | 保存并应用 | 设置页点击【保存并应用】 | 弹出 Toast 提示且主歌词窗口即时更新 |
