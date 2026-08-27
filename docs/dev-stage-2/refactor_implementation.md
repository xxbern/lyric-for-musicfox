# Lyric For Music 项目重构实施规划 (Refactor Implementation Plan)

本实施文档基于 `docs/dev-stage-2/refactor_analysis.md` 中诊断的九大核心问题，为接下来执行重构和审查的 Subagents（Worker & Reviewer）提供一份具体、严谨、可执行的规范指导。

重构的目标是建立一个高内聚、低耦合、平台无关、零冗余分配、且契合 Rust 并发特性的现代化挂件应用软件架构。

---

## 目录
1. [整体架构与数据流流向](#1-%E6%95%B4%E4%BD%93%E6%9E%B6%E6%9E%84%E4%B8%8E%E6%95%B0%E6%8D%AE%E6%B5%81%E6%B5%81%E5%90%91)
2. [P1 阶段：核心共享上下文与有界事件总线](#2-p1-%E9%98%B6%E6%AE%B5%E6%A0%B8%E5%BF%83%E5%85%B1%E4%BA%AB%E4%B8%8A%E4%B8%8B%E6%96%87%E4%B8%8E%E6%9C%89%E7%95%8C%E4%BA%8B%E4%BB%B6%E6%80%BB%E7%BA%BF)
3. [P2 阶段：平台适配接口层 (PAL) 与统一通信协议层](#3-p2-%E9%98%B6%E6%AE%B5%E5%B9%B3%E5%8F%B0%E9%80%82%E9%85%8D%E6%8E%A5%E5%8F%A3%E5%B1%82-pal-%E4%B8%8E%E7%BB%9F%E4%B8%80%E9%80%9A%E4%BF%A1%E5%8D%8F%E8%AE%AE%E5%B1%82)
4. [P3 阶段：高内聚业务服务化拆分](#4-p3-%E9%98%B6%E6%AE%B5%E9%AB%98%E5%85%A5%E5%85%B3%E4%B8%9A%E5%8A%A1%E6%9C%8D%E5%8A%A1%E5%8C%96%E6%8B%86%E5%88%86)
5. [P4 阶段：字体加载深度优化与轻量化](#5-p4-%E9%98%B6%E6%AE%B5%E5%AD%97%E4%BD%93%E5%8A%A0%E8%BD%BD%E6%B7%B1%E5%BA%A6%E4%BC%98%E5%8C%96%E4%B8%8E%E8%BD%BB%E9%87%8F%E5%8C%96)
6. [P5 阶段：简化 GUI 状态机与全链路集成验证](#6-p5-%E9%98%B6%E6%AE%B5%E7%AE%80%E5%8C%96-gui-%E7%8A%B6%E6%80%81%E6%9C%BA%E4%B8%8E%E5%85%A8%E9%93%BE%E8%B7%AF%E9%9B%86%E6%88%90%E9%AA%8C%E8%AF%81)
7. [质量与交付验收矩阵](#7-%E8%B4%A8%E9%87%8F%E4%B8%8E%E4%BA%A4%E4%BB%98%E9%AA%8C%E6%94%B6%E7%9F%A9%E9%98%B5)

---

## 1. 整体架构与数据流流向

重构后的模块架构如下图所示：

```
                    ┌────────────────────────────────────────────────────────┐
                    │                      Main Process                      │
                    │                                                        │
                    │   ┌──────────────┐         ┌────────────────────────┐  │
                    │   │   EventBus   │◄────────┤ UDP / IPC / Tray Threads│ │
                    │   └──────┬───────┘         └────────────────────────┘  │
                    │          │ (Drains per frame)                          │
                    │          ▼                                             │
 ┌─────────────┐    │   ┌──────────────┐         ┌────────────────────────┐  │
 │  Egui App   ├───►│   │  AppContext  │◄────────┤   Business Services    │  │
 │ (LyricApp)  │    │   └──────┬───────┘         │  (Config, Font, etc.)  │  │
 └──────┬──────┘    │          │                 └───────────┬────────────┘  │
        │           │          ▼                             │               │
        │           │   ┌──────────────┐                     │               │
        └───────────┼──►│  PAL (Traits)│◄────────────────────┘               │
                    │   └──────┬───────┘                                     │
                    │          │                                             │
                    └──────────┼─────────────────────────────────────────────┘
                               ▼
                    ┌──────────────────┐
                    │ Platform Backend │
                    │  (Win32 / Stub)  │
                    └──────────────────┘
```

与现有架构相比，主要有以下几大关键变化：
- **无全局静态 GUI 上下文**：废弃 `EGUI_CTX` 全局变量，非主线程的事件完全转为异步消息发送入有界事件总线 `EventBus`，由主 GUI 线程的事件循环驱动。
- **状态高度聚合**：多个独立的分散并发原语（原子变量等）收纳进统一的 `AppContext` 中，使用强类型包装。
- **平台逻辑彻底隔离**：消除业务层中的 `#[cfg(windows)]`，由平台抽象层 (PAL) 为各平台提供 trait 实现。

---

## 2. P1 阶段：核心共享上下文与有界事件总线

针对问题四（状态散落）、问题五（反向依赖）、问题八（全局状态散落）进行重构，确立应用公共基础设施。

### 2.1 涉及文件
- **新建**：`src/context/mod.rs`（应用上下文）、`src/event_bus/mod.rs`（事件总线）
- **修改**：`src/lib.rs`、`src/main.rs`、`src/window/mod.rs`、`src/tray/mod.rs` 等

### 2.2 核心代码骨架

#### 1) 事件定义与有界事件总线
```rust
// src/event_bus/mod.rs
use crossbeam_channel::{bounded, Receiver, Sender};
use crate::Config;

#[derive(Debug, Clone)]
pub enum AppEvent {
    RequestRepaint,
    ConfigReloaded(Box<Config>),
    TrayCmd(crate::tray::TrayCmd),
    LyricStateChanged,
}

#[derive(Clone)]
pub struct EventBus {
    sender: Sender<AppEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> (Self, Receiver<AppEvent>) {
        let (tx, rx) = bounded(capacity);
        (Self { sender: tx }, rx)
    }

    pub fn emit(&self, event: AppEvent) {
        // 使用 try_send 防止写阻塞，保障高能效；若满了可打日志或按需丢弃
        if let Err(e) = self.sender.try_send(event) {
            log::warn!("EventBus channel full/disconnected: {:?}", e);
        }
    }
}
```

#### 2) 控制信号结构
```rust
// src/context/mod.rs 中的 Signals 子结构
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Signals {
    pub needs_reload: AtomicBool,
    pub is_dragging: AtomicBool,
    pub display_changed: AtomicBool,
}

impl Default for Signals {
    fn default() -> Self {
        Self {
            needs_reload: AtomicBool::new(false),
            is_dragging: AtomicBool::new(false),
            display_changed: AtomicBool::new(false),
        }
    }
}
```

#### 3) 应用共享上下文
```rust
// src/context/mod.rs
use std::sync::{Arc, RwLock};
use crate::Config;
use crate::lyric::state::LyricState;
use crate::event_bus::EventBus;
use super::Signals;

pub struct AppContext {
    pub config: RwLock<Config>,
    pub state: RwLock<LyricState>,
    pub signals: Signals,
    pub event_bus: EventBus,
}

impl AppContext {
    pub fn new(config: Config, state: LyricState, event_bus: EventBus) -> Self {
        Self {
            config: RwLock::new(config),
            state: RwLock::new(state),
            signals: Signals::default(),
            event_bus,
        }
    }

    pub fn get_config(&self) -> Config {
        self.config.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn update_config(&self, new_cfg: Config) {
        {
            let mut lock = self.config.write().unwrap_or_else(|p| p.into_inner());
            *lock = new_cfg;
        }
        self.event_bus.emit(crate::event_bus::AppEvent::ConfigReloaded(Box::new(self.get_config())));
    }
}
```

### 2.3 实施步骤与逻辑改造
1. **构建模块结构**：在 `src/lib.rs` 中注册 `context` 和 `event_bus` 模块。
2. **初始化并传递**：在 `src/main.rs` 的入口处将 `EventBus` 的容量设为 `256`。初始化 `AppContext` 并在主进程与启动的各个后台线程中共享其 `Arc<AppContext>` 引用。
3. **销毁 EGUI_CTX**：将原来读取并调用 `EGUI_CTX.get().unwrap().request_repaint()` 的逻辑替换成 `ctx.event_bus.emit(AppEvent::RequestRepaint)`。
4. **主面板排空机制**：在 `LyricApp::update` 的最顶部，循环清空事件总线管道并分发处理：
   ```rust
   while let Ok(event) = self.event_bus_rx.try_recv() {
       match event {
           AppEvent::RequestRepaint => ctx.request_repaint(),
           AppEvent::ConfigReloaded(cfg) => self.reload_config(ctx, *cfg),
           AppEvent::TrayCmd(cmd) => self.handle_tray_cmd(ctx, cmd),
           AppEvent::LyricStateChanged => {
               self.scroll.needs_recompute = true;
               ctx.request_repaint();
           }
       }
   }
   ```

### 2.4 P1 验证方案
- **单元测试**：编写单元测试建立多个子线程高频写入 `EventBus`。测试在容量已满时是否会表现出预期中的无死锁、无阻塞非阻塞写入（`try_send` 失败抛出相应告警而不是卡死）。
- **流程测试**：测试通过总线派发 `ConfigReloaded`，确认 `update()` 流程能够无缝捕获并实时更新本地变量。

---

## 3. P2 阶段：平台适配接口层 (PAL) 与统一通信协议层

针对问题二（IPC 协议复制）、问题三（平台代码散落）、问题七（IPC 服务客户端混杂）进行解决。剥离原生 Win32 的紧耦合逻辑，实现 Linux 下的可测性。

### 3.1 涉及文件
- **新建**：
  - `src/platform/mod.rs`（平台适配入口）
  - `src/platform/trait.rs`（接口定义）
  - `src/platform/windows/` 与 `src/platform/stub/`（底层具体实现）
  - `src/protocol/mod.rs`（IPC 编解码器与命名规则）
- **修改**：
  - `src/pipe/windows_pipe.rs`（重构或废弃并将其拆散）、`src/main.rs` 等

### 3.2 核心接口设计

#### 1) 跨平台功能 Traits
```rust
// src/platform/trait.rs
use std::path::Path;

pub trait PlatformTray {
    fn create_tray(&self, event_bus: crate::event_bus::EventBus) -> Result<(), String>;
    fn set_tooltip(&self, text: &str) -> Result<(), String>;
    fn flash_icon(&self, running: bool) -> Result<(), String>;
}

pub trait PlatformPipeServer {
    fn start(&self, name: &str, event_bus: crate::event_bus::EventBus) -> Result<(), String>;
    fn stop(&self);
}

pub trait PlatformPipeClient {
    fn send_request(&self, pipe_name: &str, payload: &[u8], timeout_ms: u32) -> Result<Vec<u8>, String>;
}

pub trait PlatformWindowStyle {
    fn set_through(&self, hwnd: *mut std::ffi::c_void, transparent: bool);
    fn refresh_stay_on_top(&self, hwnd: *mut std::ffi::c_void);
}
```

#### 2) IPC 消息与编码协议
```rust
// src/protocol/mod.rs
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum IpcMessage {
    ReloadConfig,
    GetPosition,
    PositionResponse(Option<(i32, i32)>),
    ActivateSettings(u32),
    Success,
}

impl IpcMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::ReloadConfig => b"RELOAD\n".to_vec(),
            Self::GetPosition => b"GET_POS\n".to_vec(),
            Self::PositionResponse(Some((x, y))) => format!("POS {},{}\n", x, y).into_bytes(),
            Self::PositionResponse(None) => b"POS EMPTY\n".to_vec(),
            Self::ActivateSettings(pid) => format!("ACTIVATE {}\n", pid).into_bytes(),
            Self::Success => b"OK\n".to_vec(),
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let s = std::str::from_utf8(bytes).map_err(|e| e.to_string())?.trim();
        if s == "RELOAD" {
            return Ok(Self::ReloadConfig);
        }
        if s == "GET_POS" {
            return Ok(Self::GetPosition);
        }
        if s == "POS EMPTY" {
            return Ok(Self::PositionResponse(None));
        }
        if s.starts_with("POS ") {
            let coords = &s[4..];
            let parts: Vec<&str> = coords.split(',').collect();
            if parts.len() == 2 {
                let x = parts[0].parse::<i32>().map_err(|e| e.to_string())?;
                let y = parts[1].parse::<i32>().map_err(|e| e.to_string())?;
                return Ok(Self::PositionResponse(Some((x, y))));
            }
        }
        if s.starts_with("ACTIVATE ") {
            let pid = s[9..].parse::<u32>().map_err(|e| e.to_string())?;
            return Ok(Self::ActivateSettings(pid));
        }
        if s == "OK" {
            return Ok(Self::Success);
        }
        Err(format!("invalid protocol message: {}", s))
    }
}

pub fn get_pipe_path(base: &str, session_id: u32) -> String {
    format!(r"\\.\pipe\lyric-for-musicfox-{}-{}", base, session_id)
}
```

### 3.3 实施步骤与移植
1. **实现平台抽象**：
   - 在 `src/platform/windows/` 目录下放置所有使用 `windows` 依赖包和 Win32 原生操作的具体类。
   - 在 `src/platform/stub/` 下提供结构一致的 Stub No-op 实现类，完全剥离 Windows-specific API。
2. **提取会话标识获取**：
   - 命名管道名使用的 `get_session_id()` 函数归档进入统一的 `src/protocol/mod.rs`（或在其下通过 Windows APIs 的封装，同时在平台包注入，在 Linux 下只返回一个固定常量如 `0`，使得单测完全不会报错）。
3. **消除 cfg 污染**：
   - 提取原本分散在 `window/mod.rs`、`tray/mod.rs` 等文件中的 `#[cfg(windows)]` 条件属性。
   - 替换为使用类似 `src::platform::current_platform()` 获取具体的 Trait 实现句柄来发号施令。
4. **管道业务解耦**：
   - 重构 `src/pipe` 功能，原本混杂的 Named Pipe 大循环使用 PAL 封装出来的管道服务代替。
   - 确保 `reload`、`pos`、`presence` 管道消息解析使用 `IpcMessage::parse` 进行解开，从逻辑上和 IO 处理彻底隔离。

### 3.4 P2 验证方案
- **单元测试**：可在 Linux 平台（及在 GitHub Actions/Linux CI 环境）直接跑 `src/protocol/mod.rs` 下的所有编解码单元测试（检测 `IpcMessage::parse` 对输入如 `POS 100,200`、`POS EMPTY` 以及不符合规范字符的处理）。
- **一致性检验**：确保重新构建后，生成的 Windows 名命名管道地址和行为仍与原版系统（特别是与旧版配置端和外部 patching）完全兼容。

---

## 4. P3 阶段：高内聚业务服务化拆分

解决问题六（业务逻辑和渲染纠缠在一起），精简庞大的主文件 `window/mod.rs`。

### 4.1 涉及文件
- **新建**：
  - `src/services/mod.rs`
  - `src/services/config.rs`（配置服务）
  - `src/services/font.rs`（字体生命周期服务）
  - `src/services/monitor.rs`（坐标计算裁剪服务）
  - `src/services/position.rs`（窗口位置与拖拽状态服务）
  - `src/services/style.rs`（外观与 Win32 穿透修饰服务）
  - `src/services/tray.rs`（托盘控制服务）
  - `src/services/udp.rs`（UDP数据监听服务）
- **修改**：
  - `src/window/mod.rs` 及业务引用者

### 4.2 业务聚合服务 handles 设计
```rust
// src/services/mod.rs
use std::sync::Arc;
use crate::context::AppContext;

pub struct ServiceHandles {
    pub config: ConfigService,
    pub font: FontService,
    pub monitor: MonitorService,
    pub position: PositionService,
    pub style: WindowStyleService,
    pub tray: TrayService,
    pub udp: LyricUdpService,
}

impl ServiceHandles {
    pub fn new(ctx: Arc<AppContext>) -> Self {
        Self {
            config: ConfigService::new(ctx.clone()),
            font: FontService::new(ctx.clone()),
            monitor: MonitorService::new(ctx.clone()),
            position: PositionService::new(ctx.clone()),
            style: WindowStyleService::new(ctx.clone()),
            tray: TrayService::new(ctx.clone()),
            udp: LyricUdpService::new(ctx),
        }
    }
}
```

### 4.3 各独立服务模块职责与核心方法规划

| 服务名称 | 对应职责 | 暴露的关键接口方法 |
|---|---|---|
| **ConfigService** | 配置文件磁盘读写、校验、广播热重载 | `load_or_default()`<br>`save(&self, cfg: Config)`<br>`reload_from_disk(&self)` |
| **FontService** | 字体枚举定位、获取样式字节包、绑定 GPU | `resolve_family(&self, requested: &str) -> String`<br>`get_font_bytes(&self, family: &str, bold: bool, italic: bool) -> Option<Vec<u8>>`<br>`apply_to_egui(&self, egui_ctx: &egui::Context)` |
| **MonitorService** | 多显示器几何布局查询、越界坐标强制收拢 | `enumerate_screens(&self) -> Vec<ScreenLayout>`<br>`clamp_position(&self, original: (i32, i32), width: u32, height: u32) -> (i32, i32)` |
| **PositionService** | 封装拖拽状态机状态流转、最终窗口坐标存取 | `handle_drag_start(&self, phys_ptr: (i32, i32))`<br>`update_drag_position(&self, phys_ptr: (i32, i32)) -> Option<(i32, i32)>`<br>`get_current_pos(&self) -> (i32, i32)` |
| **WindowStyleService**| 实现鼠标穿透锁定和前台 Stay-on-top 定时刷新 | `set_click_through(&self, locked: bool)`<br>`enforce_always_on_top(&self)` |
| **TrayService** | 提供操作系统托盘对象的构建与气泡菜单管理 | `init_tray(&self)`<br>`destroy_tray(&self)` |
| **LyricUdpService** | 管理接收底层歌词解析线程的生命周期与启停 | `start_listening(&self, port: u16)`<br>`stop_listening(&self)` |

### 4.4 实施步骤
1. **分解代码**：逐一将 `src/window/mod.rs` 中大段的显示器裁剪（`check_display_change` 等）、拖拽样式注入、字体回退选择复制提取至对应的业务服务中。
2. **状态注入**：各业务服务统一仅在内部保存 `Arc<AppContext>`。通过对 `AppContext` 状态读写以及向事件总线派发事件完成交互，彻底切断服务之间的紧密耦合网状调用。
3. **注册与加载**：在 `run` 函数的核心启动流程中构建 `ServiceHandles` 统一实例。

### 4.5 P3 验证方案
- **高度单元测试化**：剥离了 UI 渲染后。为 `MonitorService` 编写单测，针对“超出显示器布局的初始坐标 (例如 `x: -9999, y: 9999`)”输入，确保能够百分之百裁剪进主物理显示器的有效渲染坐标区。
- **状态一致性保护**：验证 `PositionService` 在不同的鼠标拖动状态变迁（`Start`, `Active`, `End`）下，内存里的坐标是否和本地持久化状态数据完全吻合。

---

## 5. P4 阶段：字体加载深度优化与轻量化

针对问题一（字体加载内存膨胀）进行专项系统调优。

### 5.1 涉及文件
- **修改**：`src/services/font.rs`

### 5.2 精准设计细节

#### 1) 系统字体数据库 OnceLock 全局共享化
`fontdb::Database` 内存元数据约占 15MB~30MB。我们使用 `Result` 和 `OnceLock` 容器将其生命周期改造为进程单例，避免任何重复读取行为。
```rust
// src/services/font.rs 中
use std::sync::OnceLock;

static SYSTEM_FONTS_DB: OnceLock<fontdb::Database> = OnceLock::new();

pub fn get_system_fonts_ref() -> &'static fontdb::Database {
    SYSTEM_FONTS_DB.get_or_init(|| {
        log::info!("Scanning system fonts database (happens only once per process)...");
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}
```

#### 2) `Vec<u8>` 提取后即刻回收模式
在 `FontService` 为 `egui` 绑定配置的中文字体字节完毕后，通过作用域生命周期将庞大的原始载荷字节进行物理释放（从 Heap 销毁），不存留在常驻逻辑的对象字段中。
```rust
// src/services/font.rs
use egui::{FontData, FontDefinitions, FontFamily};

pub fn apply_selected_font(ctx: &egui::Context, family_name: &str, bold: bool, italic: bool) {
    let db = get_system_fonts_ref();
    
    // 1. 优先提取对应特征字体的字节载荷
    let bytes_opt = crate::window::load_font_bytes(db, family_name, bold, italic);
    
    if let Some(bytes) = bytes_opt {
        // 2. 清退 egui 内置默认字体，防止额外的内存堆积
        let mut fonts = FontDefinitions::empty();
        
        // 3. 构建一次性的 Owned Data 写入
        fonts.font_data.insert(
            family_name.to_owned(),
            FontData::from_owned(bytes) // 此处的 bytes 分配在下面 fonts 设置进 egui 之后即被回收
        );
        
        fonts.families.insert(
            FontFamily::Proportional,
            vec![family_name.to_owned()],
        );
        
        ctx.set_fonts(fonts);
        log::info!("Successfully applied user custom font family: {}", family_name);
    } else {
        log::warn!("Failed to fetch font bytes for: {}, keeping defaults", family_name);
    }
    // 注意：在这里 bytes 离开作用域，系统底层的大块多余 Vec<u8> 得以被 Rust GC 直接回收
}
```

### 5.3 实施步骤与优化
1. **全局数据库重构**：将所有使用到 `fontdb::Database::new()` 的代码行废弃，全部替换为 `get_system_fonts_ref()`。
2. **清理多余项**：删除 `LyricApp` 中的 `font_db` 与中间变量字段。
3. **清除默认字体**：使用 `FontDefinitions::empty()` 清理内部可能驻留的 NotoEmoji、EmojiOne 等字体库，只包含用户指定的中文字体样式。

### 5.4 P4 验证方案
- **冷热启动内存监控**（针对 Windows 目标程序）：
  - 记录刚启动 5 秒的内存指标。
  - 触发“连续保存配置 10 次”触发 10 次字体刷新。检查工作集内存，绝对不能产生阶梯上升（代表无内存碎片泄露，分配的 `Vec<u8>` 正常被回收释放）。
  - 稳态内存必须长期维持在 **< 30MB~50MB** 的水位线上。

---

## 6. P5 阶段：简化 GUI 状态机与全链路集成验证

解决问题九（LyricApp 字段过多且全部公开）并进行终期总检。

### 6.1 涉及文件
- **修改**：`src/window/mod.rs`（主窗口容器重塑）、`src/lib.rs` 等全部入口

### 6.2 GUI 主控制体全面重构
将 `LyricApp` 的 25+ 暴露字段大幅压缩整合为职责划分明确的 5 个内部子模块。

```rust
// src/window/mod.rs
use std::sync::Arc;
use crate::context::AppContext;
use crate::services::ServiceHandles;

pub struct LyricApp {
    // 1. 公共上下文状态
    ctx: Arc<AppContext>,
    
    // 2. 外部业务调用接口 handle
    services: ServiceHandles,
    
    // 3. 渲染数据流高速缓存
    render_cache: RenderCache,
    
    // 4. 定时滚动进度机
    scroll: crate::window::scroll::ScrollState,
    
    // 5. 悬浮窗拖拽器
    drag: crate::window::drag::DragState,
}
```

其中把原本散布的 `cached_*` 等与渲染密切关联的方法字段，高内聚低耦合地划入 `RenderCache`：
```rust
// src/window/render_cache.rs
use std::sync::Arc;

pub struct RenderCache {
    galley: Option<Arc<egui::Galley>>,
    text: String,
    font_family: String,
    font_size: f32,
    bold: bool,
    italic: bool,
    playing: bool,
    is_placeholder: bool,
    window_width: f32,
}

impl RenderCache {
    pub fn new() -> Self {
        Self {
            galley: None,
            text: String::new(),
            font_family: String::new(),
            font_size: 0.0,
            bold: false,
            italic: false,
            playing: false,
            is_placeholder: false,
            window_width: 0.0,
        }
    }

    pub fn should_invalidate(
        &self,
        new_text: &str,
        font_family: &str,
        font_size: f32,
        bold: bool,
        italic: bool,
        playing: bool,
        is_placeholder: bool,
        window_width: f32,
    ) -> bool {
        self.galley.is_none()
            || self.text != new_text
            || self.font_family != font_family
            || (self.font_size - font_size).abs() > f32::EPSILON
            || self.bold != bold
            || self.italic != italic
            || self.playing != playing
            || self.is_placeholder != is_placeholder
            || (self.window_width - window_width).abs() > f32::EPSILON
    }

    pub fn get_galley(
        &mut self,
        painter: &egui::Painter,
        new_text: &str,
        style: &crate::config::LyricStyle,
        font_family: &str,
        is_placeholder: bool,
        playing: bool,
        window_width: f32,
    ) -> Arc<egui::Galley> {
        if self.should_invalidate(
            new_text,
            font_family,
            style.font_size,
            style.font_bold,
            style.font_italic,
            playing,
            is_placeholder,
            window_width,
        ) {
            let new_galley = crate::window::render::layout_lyric(
                painter,
                new_text,
                style,
                font_family,
                is_placeholder,
                playing,
            );
            self.galley = Some(new_galley.clone());
            self.text = new_text.to_string();
            self.font_family = font_family.to_string();
            self.font_size = style.font_size;
            self.bold = style.font_bold;
            self.italic = style.font_italic;
            self.playing = playing;
            self.is_placeholder = is_placeholder;
            self.window_width = window_width;
            new_galley
        } else {
            self.galley.as_ref().unwrap().clone()
        }
    }
}
```

### 6.3 实施步骤与调整
1. **隐藏可见性**：把 `LyricApp` 下属的所有成员变量由原本的 `pub` 更新为完全的 `private`。
2. **集成适配器**：改写 `LyricApp` 内部的 `new()` 构造函数，用精简的字段和辅助模块对齐。
3. **完成最终组装**：通过 `cargo test` 跑通测试，保证全部测试项目（如 `tests/drag_test.rs` 等）适配新的 `AppContext` / `ServiceHandles` 架构调用。

---

## 7. 质量与交付验收矩阵

以下为本次重构交付验收必须全量通过的检查项：

| 校验分类 | 检测指标与命令 | 评定基准和标准 |
|---|---|---|
| **跨系统编译** | `cargo build --target x86_64-pc-windows-gnu` | 须 100% 成功编译，保证交叉编译正常运作。 |
| **可执行文件大小限制**| 检查生成的产物大小 | 无意外膨胀，不应超过既定的发行二进制体积界限。 |
| **PE 资源嵌入性**| `objdump -p target/x86_64-pc-windows-gnu/debug/lyric-for-musicfox.exe \| grep -A 5 "Resource Directory"` | 检查结果中资源表必须保持完整，确保 `assets/icon.rc` 没有受影响。 |
| **自动化测试集**| `cargo test --all` | 所有已编写的单元测试与集成测试通过，没有功能性滑坡 regression。 |
| **常驻内存（Win32）**| 悬浮窗冷启动 5 秒后的 RAM 水平 | 常态静止运行 5-10 分钟，内存维持在 **30MB~50MB** 的严格范围之内。 |
| **单元测试隔离性**| CI 系统在 Linux 环境下直接运行全部测试 | 无需真实的 Win32 系统环境与设备接口，全部核心 IPC/配置/拖动的行为能被模拟跑过编译。 |
| **日志线程关闭** | 查看磁盘产生 | 默认不产生多余的 write I/O 开销与 `log.txt` 数据。 |

---
> **后续执行建议**：本实施文档可作为 Subagent 分部执行的具体路线图。在推进具体模块的逻辑提取与重构时，可通过 Scout 判断所处的文件结构变化，Worker 编写代码逻辑，Oracle 或 Reviewer 全程核实各模块的数据一致性。
