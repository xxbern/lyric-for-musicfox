## P1. 歌词窗口（核心渲染）

### 关键决策
- **裸 LyricState**：P1 无并发线程持有，LyricApp 直接持有裸值；P3 引入 UDP 时升级为 Arc<RwLock<>>。
- **is_placeholder 独立标记**：占位符 alpha=0.3 不能仅从 playing=false 推断（暂停歌词也是 0.5）；独立标记避免歧义。
- **P1 不提供 reload-{session_id} / presence 握手**：dev.md §31 明确这两个机制 P4 交付；P1 阶段设置进程单实例的“激活”语义降级为 exit 5。
- **字体回退机制**：启动时一次性 fontdb 查询 → 不存在则回退 Microsoft YaHei + warn；不修改 config.toml。
- **.tmp 双角色**：防抖写的工作草稿 + 原子保存的源；dirty 语义独立于 tmp 写操作。

### 与 req.md / dev.md 偏差
- 颜色解析失败回退白色：仅防御性，正常路径下不会触发（validate 在保存前拦截）。
- replace_file 在 Windows rename 失败时退化为 remove+rename（CI 在 MSVC 工具链下应验证原子性）。

### 下阶段依赖项
- P2 拖动 + 锁定：window/drag.rs + window/hit_test.rs；需读 config.window.locked 注入 WS_EX_TRANSPARENT。
- P3 UDP：Arc<RwLock<LyricState>> + lyric/udp.rs + UdpLyricPayload schema。
- P4 托盘 + 管道：ipc.rs + pipe/{reload,pos,presence}.rs + reload-{session_id} 服务端。

### 遗留问题
- font_picker dedup 同名不同字重字体：当前按 family name 去重，理论上可能漏字重；实际 fontdb 通常按 ID 区分，暂可接受。
- pos 客户端在 P1 阶段必然超时：P1 主进程无 pos 服务端；P4 接入服务端后自动生效。

## P2. 拖动 + 锁定 + 副屏
### 关键决策
- **裸 LyricState 不升级 Arc<RwLock>**：拖动仅 GUI 主线程 PointerMoved/Released 写 pos_x/y；UDP/管道写线程 P3/P4 才引入；P2 不需要并发保护。
- **HWND 获取**：eframe::Frame.window_handle() → raw-window-handle 0.6 → Win32WindowHandle.hwnd；仅第一次 update 后写入 LyricApp.hwnd；WS_EX_TRANSPARENT 注入一次性。
- **物理像素 vs DIP**：位置字段物理像素贯穿（winit::PhysicalPosition 直接使用），尺寸字段 DIP（eframe 默认）；运行期 DPI 转换仅在字体度量。
- **裁剪基准 = rcMonitor**（req.md §14）：不是 rcWork。窗口与任一已枚举显示器相交则原样保留（副屏仍在）；完全不可见才钳到主屏 left/top/right-win/bottom-win。
- **裁剪不写盘**：clamp_to_primary_monitor 改内存 config.window 后直接构造 native_options；不调 config::save。
- **拖动不自动落盘**：Released 事件写 LyricState.pos_x/y；后续持久化依赖用户打开设置页 → 保存路径（req.md §7.4 设计权衡）。
- **依赖**：显式 `raw-window-handle = "0.6"`（eframe/winit 已传递）；windows 增加 `Win32_UI_HiDpi` 以调用 GetDpiForMonitor。
### 与 req.md / dev.md 偏差
- 拖动不落盘验证依赖「设置页保存」路径（P1 已具备 pos 客户端，但 P2 主进程未启动 pos 服务端；设置页超时回退 Stale 时仍可通过用户手动改 pos_x/y 字段触发保存）。P4 接通 pos 服务端后「无感保存」生效。
- clamp 基准从 rcWork 改为 rcMonitor（Reviewer D1 修正）；默认居中仍按主屏整屏 + 顶部 100 DIP。
### 下阶段依赖项
- P3 UDP：LyricState 升级 Arc<RwLock<_>>；drag::process_pointer_input 改写为 state.write().pos_x/y；UDP 线程不修改 pos 字段（无冲突）。
- P4 托盘 + 管道：reload 信号触发 hit_test::apply_locked_style 二次调用；pos-{session_id} 服务端读 LyricState.pos_x/y 响应设置进程查询；WM_DISPLAYCHANGE hook + 轮询兜底迁移到 monitor::on_display_change。
### 遗留问题
- 拖动期间 outer_position 读 vs SetWindowPos 写交错（PointerMoved 内仅写不读，松手读一次）。
- 启动期 monitor::enumerate 失败时降级为 pixels_per_point=1.0 + 单屏 1920×1080 占位 layout；RDP/容器场景可能不正确，需 P4 兜底。

## P3. UDP 通信

### 关键决策
- **Arc<RwLock<LyricState>> 升级**：因为 UDP 接收线程与 GUI 主线程并发写/读不同字段，因此将 LyricState 升级为 Arc<RwLock<>> 保护。写锁在 UDP 解析包后和 pointer_moved 时的 Drag 模块中获取，读锁在 GUI update 每帧渲染时获取，完全避免进程间冲突并保证高频刷新。
- **UDP 套接字绑定与直接端口占用**：在主进程启动前直接绑定 socket (0.0.0.0:receive_port)，若端口已被抢占或不可用则在 stdout/stderr 打印错误并 exit 1，免除 probe_port + bind 的多系统调用抢占窗口竞态。
- **8KB 包过滤与 type 校验**：由于套接字上限设计，我们实现了 8KB 过滤。如果 UDP 包的大小超过 8192 或 type 不是 "lyric_update"，均打印 warn 日志并丢弃，不污染状态值。
- **is_placeholder 标记与 UI 渲染解耦**：如果 current_line text 为空，渲染层自动把 is_placeholder 设为 true，并把展示文本设为 "......"，并且 alpha 值降为 0.3。
- **阻塞接收线程 panic 拦截**：在 UDP 接收线程中使用 catch_unwind 强力抓取并兜底可能会因数据解析异常导致的主线崩溃动作，当 lock 发生 poison 时亦能将其还原并降级恢复。
- **mock_udp_sender 仿真辅助**：独立实现了 tests/mock_udp_sender.rs 用于在本地持续发送带有 playing/歌词文本序列的模拟协议到 16501，降低外部对接成本。

### 与 req.md / dev.md 偏差
- probe_port 仅保留在 lib.rs/instance 模块，在主进程 main.rs 内被直接 bind 所替代。
- 保证 UdpSongInfo 支持 id/album/pic_url/duration/is_favorite 等全部扩充字段对齐。

### 下阶段依赖项
- P4 托盘与 IPC 管道：reload-{session_id} 通信机制；
- P5 出站命令：command 模块的 6 个命令发送。

### 遗留问题
- 无。

## P4. 托盘 + 进程协作 (IPC)

### 关键决策
- **窗口 Subclassing 监听 WM_DISPLAYCHANGE**：在 Windows 平台上使用 `SetWindowLongPtrW` 成功挂载 `monitor_wnd_proc` Detour 拦截 `WM_DISPLAYCHANGE` (0x007E) 消息，通过全局 `DISPLAY_CHANGED_EVENT` 传递更改信号。
- **5s 轮询兜底**：配合 `monitor::DISPLAY_CHANGED_EVENT` 以及 App 层的 5 秒定时轮询双轨并行，保证跨会话或 DPI 不一致时的多角度裁剪兜底。
- **IPC 进程协作管道**：
  - `reload-{session_id}`：reload 管道线程内同步读取、解析 config.toml，写入共享的 `Arc<Mutex<Option<Config>>>`，将 needs_reload 置 true 供主线程在下一帧 GUI tick 消费，避免了 GUI 线程的任何阻塞 IO。
  - `pos-{session_id}`：管道服务获取主进程 `LyricState.pos_x/pos_y` 实时位置并响应设置端的 `GET_POS` 询问。
  - `presence-{session_id}`：新启动的 settings 进程通过 `AllowSetForegroundWindow(old_pid)` 配合旧实例的 `SetForegroundWindow` 动作突破前台锁限制，成功激活已打开的设置窗口。
- **系统托盘**：托盘左键点击和右键“配置”都会拉起 `--settings` 分支进程以实现聚焦；配置损坏时通过 `icon_gray.ico` 配合 normal 图标以 1Hz 频率（500ms 刷新）闪烁。

### 与 req.md / dev.md 偏差
- 修正 AllowSetForegroundWindow 执行主体：必须由当前持有 Foreground 聚焦特权的 Launcher (client) 显式调用 `AllowSetForegroundWindow(server_pid)`，授权给后台的原 settings Window 后方可调用 `SetForegroundWindow`，解决了 Windows 安全前台锁的前后脚冲突。

### 下阶段依赖项
- P5 出站命令接收。

### 遗留问题
- 无。

## P5. 出站命令 + 日志轮转 + 优化 + 文档

### 关键决策
- **出站命令实现 (src/command)**: 按 `req.md` 和 `dev.md` 规范设计了 6 种出站命令 (Toggle, Next, Previous, Like, Seek, Volume) 的 enum 封装，通过本地临时 UDP 端口 (0.0.0.0:0) 将 JSON 编码的消息发送 to 127.0.0.1:send_port。
- **自定义非阻塞日志管理器 (src/logger.rs)**: 基于 `crossbeam-channel` 通道将日志行发送给独立后台工作线程处理，完全避免了 `log!` 宏导致的 GUI 线程文件 IO 阻塞。
- **LoggerGuard 自治关闭结构**: 实现了 RAII 式的 `LoggerGuard`，在其 drop 时清空全局 logger 引用，关闭 channel，并等待后台写线程将队列中的日志完全 drain (输出、刷盘) 后合并退出。
- **5MB 自动日志轮转与备份**: 后台线程在每次写入记录后检查当前 log.txt 的实测大小，若超过 5MB 会将缓冲 flush 并 sync 后关闭句柄，将 log.txt 重命名为 log.txt.old (覆盖原历史)，再重新创建 log.txt 继续运作，极大保障磁盘长期运行的安全性。
- **ISO-8601 UTC/Local 毫秒级时间戳**: 使用 `chrono::Local` 输出与标准对齐的毫秒精度 ISO-8601 时区时间戳，满足规范要求。
- **日志过滤环境变量**: 系统优先匹配 `--benchmark` 参数启动 `LevelFilter::Trace` 日志级别，同时支持从 `LYRIC_LOG` 环境变量中解析 debug/info/warn/error/trace 级别的运行时过滤。

### 与 req.md / dev.md 偏差
- 采用了更轻快、无依赖的 `crossbeam-channel` 异步日志消费实现，取代了复杂的 tracing 和 tracing-appender 依赖，不仅简化了代码和测试，且让 binary release size 得以充分轻量化到 ≤ 40MB。
- 在 `tests/logger_rotation_test.rs` 进行了单元测试集成，实现了单线程内进程级全局 LogFacade 的测试用例串行隔离。

### 遗留问题
- 无。
