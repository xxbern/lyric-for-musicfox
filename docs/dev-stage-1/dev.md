# 开发设计文档（dev.md）

> 版本：0.2.0
> 范围：实现期设计（与 `req.md` 配合：`req.md` 定义做什么，`dev.md` 定义怎么做、什么顺序做）

---

## 1. 设计目标

1. **阶段可运行**：每个阶段结束都有一个可双击启动的 `lyric-for-musicfox.exe`，功能分批补齐
2. **高内聚低耦合**：模块边界清晰、依赖单向无环、数据流单向可追踪
3. **进程内零阻塞**：GUI 主线程只做渲染和事件分发；UDP / 管道 / 显示器监控全部独立线程
4. **配置/状态分离**：配置文件只由设置进程写；运行时位置由主进程内存独占
5. **崩溃可恢复**：设置进程崩了保留 `.tmp`；主进程崩了由 OS 回收 Mutex + socket

---

## 2. 阶段规划

### P0. 骨架（CLI + 配置 + 单实例）

**目标**：能启动、参数解析、单实例互斥、配置文件读写

**包含模块**：`main`、`cli`、`config/`、`instance/`、`path`、`error`、`logger`（基础）

**交付验证**：
- `lyric-for-musicfox.exe --help` 打印帮助
- `lyric-for-musicfox.exe --version` 打印版本
- 双击 exe → 启动后打印 `P0 占位：3 秒后退出`，3 秒后正常退出（exit code 0），日志记录"启动/退出"。**P0 不提供 GUI 演示**，仅验证 CLI + 单实例 + 配置加载/保存链路
- 启动两个 exe → 第二个检测到 Mutex 存在后退出（exit code 6，见 §9），**stderr 输出一行 `"another instance running\n"`**（与 req.md §15 验收项一致）
- P0 **不启动 settings 进程**：P0 无设置 GUI、无 presence 管道，因此不验收 `--settings` 的单实例/激活行为；`--settings` 在 P0 返回 stderr `settings mode is available from P1` + exit code 4。P1 实现单个设置窗口；P4 再交付 presence 激活：成功 exit 0，失败 exit 5。
- 删除 `%APPDATA%/lyric-for-musicfox/config.toml` 后启动 exe → 检测到文件不存在 → **静默用 `Config::default()` 运行**，**不写配置文件**（主进程不负责写配置；config.toml 生成是设置进程的职责）。`Config::default()` 调用由 `cargo test` 验证「字段缺失→默认值」链路。
- `config::save` 在 P0 作为**库 API + 单元测试**交付：以临时目录验证 `Config::default()` 原子保存后可加载且字节中 Option None 输出空串；P0 主进程**不得调用**该写入 API（配置唯一写入者仍从 P1 的 settings 进程开始）。
- 故意写坏 `config.toml` → 启动 exe → **stderr 报错 + exit code 1**（不弹对话框；P1 才会接设置进程 UI）

**未实现**：所有 UI、UDP、托盘、管道

---

### P1. 歌词窗口（核心渲染）

**目标**：透明窗口 + 字体渲染 + 占位符 + 横向滚动

**包含模块**：`window/`（除 `drag.rs`、`hit_test.rs`）、`lyric/state.rs`（无 UDP，纯内存初始化）、`settings/`（设置进程的 eframe GUI + Config 读写 + pos-{session_id} 管道客户端）

**交付验证**：
- 双击 → 出现透明顶层窗口，显示 `"......"` 占位符
- 窗口默认 800×80，居中（pos_x = None）
- 启动设置进程 → 表单可改 width / height / 字号 / 颜色 / 字体 / 描边 / 加粗 / 斜体 → 保存后**重启主进程**（P1 阶段无 reload-{session_id} 管道；样式生效靠启动时重读 config.toml），重启后验证窗口样式变化
- 文本宽度 > 窗口宽度时自动横向滚动（30 DIP/秒）
- DPI 切换（**仅主屏**：主屏 100%↔150% 缩放下窗口位置与字体度量重测正常；副屏 DPI 在 P2 才处理）
- **字体回退检测**：主进程启动时主动调 fontdb 检查 config.font_family 是否存在，不存在则回退 Microsoft YaHei + warn 日志（不等到首次渲染才报错）
- **锁定 / locked 设置**：设置 UI 会显示 `locked` 复选框，但 P1 主进程**不应用锁定行为**（locked 字段保存到 config.toml 但不影响交互）；锁定是 P2 交付的一部分。验收时验证「保存 locked=true → 重启 → P2 生效前交互行为不变」

**未实现**：拖动、锁定、UDP 接收、托盘、副屏坐标裁剪

---

### P2. 拖动 + 锁定 + 副屏

**目标**：鼠标拖动窗口、locked 切换、虚拟桌面坐标、副屏支持

**包含模块**：`window/drag.rs`、`window/hit_test.rs`、`window/monitor.rs`（**静态**多屏枚举 + 主屏 DPI 适配 + 虚拟桌面坐标）
**未交付**：动态显示器插拔（依赖 winit 窗口过程 hook WM_DISPLAYCHANGE）→ P4 阶段

**交付验证**：
- 拖动歌词窗口到屏幕任意位置，松开后位置保留（仅内存）
- 设置页改 `locked = true` → 鼠标点击/拖动完全无响应、事件穿透到下层应用
- 拖到副屏 → 重启后窗口出现在副屏原位置（虚拟桌面绝对坐标）
- 副屏拔除后启动 → 窗口裁剪到主屏可见区，不写盘
- DPI 切换后窗口位置保留、字体度量重测 ≤ 50ms

**未实现**：UDP、托盘、管道

---

### P3. UDP 通信

**目标**：接收 go-musicfox 推送的歌词，实时更新

**包含模块**：`lyric/udp.rs`、网络线程

**交付验证**：
- **mock UDP 发送方**（位于 `tests/mock_udp_sender.rs`）：P3 阶段附带一个独立的 mock 工具，发送符合 `UdpLyricPayload` schema 的 JSON 包到 `127.0.0.1:16501`；验收时无需 go-musicfox
- go-musicfox 启动并播放后，歌词实时更新到歌词窗口
- 切歌时新文本从右边缘外进入（offset_x 重置）
- 暂停时文本 alpha 降到 0.5、滚动继续
- UDP 包丢失 100ms 不影响视觉连续
- 8KB 以上 UDP 包被丢弃 + warn 日志
- type ≠ "lyric_update" 丢弃 + warn

**未实现**：托盘、管道、出站

---

### P4. 托盘 + 进程协作

**目标**：tray-icon + IPC 管道全打通

**包含模块**：`tray/`、`pipe/`、`ipc.rs`、`command/`

**交付验证**：
- 双击主进程 → 托盘出现
- 左键托盘 → 启动/激活设置进程
- 右键菜单 → 配置 / 退出
- 设置页保存后 ≤ 1s 内主进程样式更新（reload-{session_id} 管道）
- 拖动窗口后打开设置页 → 位置显示主进程最新值（pos-{session_id} 管道）
- 启动第二个设置进程 → 旧实例激活（presence-{session_id} 管道 + AllowSetForegroundWindow 握手）
- 显示器插拔/分辨率变化 → 窗口坐标自动调整
- 重载期间收到新信号 → 丢弃

**未实现**：出站命令、日志滚动优化、release profile 调优

---

### P5. 出站 + 收尾

**目标**：6 个出站命令 + 日志滚动 + 启动时间 + release 调优 + 文档

**包含模块**：`command/udp_send.rs`、日志轮转

**交付验证**：
- 6 个出站命令通过内部 API 可调用（UI 入口暂未提供，但命令链路打通）
- 日志超过 5MB 自动重命名为 `log.txt.old` + 新建 `log.txt`
- 启动时间 ≤ 1s（进程开始到 eframe App update 第一次调用完成）
- exe 体积 ≤ 40 MB（release profile：`lto = true`、`codegen-units = 1`、`panic = "unwind"`、`strip = true`、`opt-level = 3`）
- `IMPLEMENTATION_NOTES.md` 记录所有实现期决策
- `README.md` 含使用说明

---

## 3. 模块结构

```
lyric-for-musicfox/
├── Cargo.toml
├── build.rs                    # winres 嵌入 .ico
├── assets/
│   ├── icon.ico                # 正常托盘图标
│   └── icon_gray.ico           # 解析失败闪烁用
├── docs/
│   ├── ARCHITECTURE.md         # 模块依赖图、数据流、并发模型（实施时细化）
│   ├── IMPLEMENTATION_NOTES.md # 实现期决策记录
│   └── PROTOCOL.md             # UDP 协议细节
├── tests/
│   └── mock_udp_sender.rs      # P3 mock UDP 发送工具（发送符合 UdpLyricPayload 的 JSON 到 127.0.0.1:16501，用于验收无需 go-musicfox）
├── src/
│   ├── main.rs                 # 入口：CLI 解析 + 进程分流
│   ├── cli.rs                  # 命令行参数定义（clap）
│   ├── config/
│   │   ├── mod.rs              # Config struct + 校验 + 默认值
│   │   ├── load.rs             # TOML 加载、字段缺失策略、解析失败兜底
│   │   └── save.rs             # 原子写（.tmp → rename）、.bak 备份
│   ├── instance/
│   │   ├── mod.rs              # 单实例协调
│   │   ├── mutex.rs            # 主/设进程 Mutex（Local 命名空间）
│   │   └── port.rs             # UDP 端口 bind 探测
│   ├── pipe/
│   │   ├── mod.rs              # 命名管道抽象（连接/断开/IO 模式）
│   │   ├── reload.rs           # reload-{session_id} 管道（设置→主，单向）
│   │   ├── pos.rs              # pos-{session_id} 管道（设置问→主答，双向）
│   │   └── presence.rs         # presence-{session_id}（设置进程间双向激活握手）
│   ├── ipc.rs                  # IPC 协调：连接所有管道、订阅 state 变更
│   ├── window/
│   │   ├── mod.rs              # 窗口生命周期、eframe 集成
│   │   ├── render.rs           # 歌词文本渲染（占位 / 居中 / 滚动）
│   │   ├── scroll.rs           # 滚动状态机（offset_x 累积、循环）
│   │   ├── drag.rs             # 拖动 + 锁定（WS_EX_TRANSPARENT 切换）
│   │   ├── hit_test.rs         # 整窗口 hit-test（locked=true 注入）
│   │   └── monitor.rs          # 显示器枚举、副屏坐标、DPI 适配
│   ├── lyric/
│   │   ├── mod.rs              # 歌词状态聚合
│   │   ├── state.rs            # 内存状态（current_line / next_line / playing）
│   │   └── udp.rs              # UDP 监听 + JSON 反序列化（独立线程）
│   ├── command/
│   │   ├── mod.rs              # Command enum + dispatch（6 个出站命令的顶层封装）
│   │   └── udp_send.rs         # UDP 发送（OS 临时端口、JSON 序列化、失败仅日志）
│   ├── tray/
│   │   ├── mod.rs              # tray-icon 集成
│   │   ├── icon.rs             # 图标资源加载（从 PE 资源 IDI_ICON1 / IDI_ICON1_GRAY；build.rs 嵌入 assets/icon.ico + icon_gray.ico 两个 .ico）
│   │   └── menu.rs             # 右键菜单
│   ├── settings/
│   │   ├── mod.rs              # 设置进程入口
│   │   ├── ui.rs               # egui 表单 UI（主窗口 + 模态对话框）
│   │   ├── form.rs             # 表单状态 + 防抖（500ms）
│   │   ├── validate.rs         # 字段校验（颜色/范围/字体存在）
│   │   ├── font_picker.rs      # 系统字体枚举 + egui ComboBox
│   │   └── color_picker.rs     # 颜色选择器（输入框 + 预览）
│   ├── logger.rs               # tracing 初始化 + 滚动策略（5MB）
│   ├── path.rs                 # %APPDATA%/lyric-for-musicfox/ 路径解析
│   └── error.rs                # 统一错误类型（thiserror）
└── README.md
```

---

## 4. 模块依赖图

```
                            main
                              │
                ┌─────────────┼─────────────┐
                │             │             │
               cli        instance       window (P1+)
                │             │             │
                │       ┌─────┴─────┐      lyric (P3+)
                │       │           │       │
                │     mutex       port     state
                │                             │
                │                             udp
                │                             │
                │                         command (P5)
                │
      ┌─────────┴────────┐
      │                  │
  config/             logger/ ──── (所有模块)
      │
    path

  pipe/ ────┐
            ├── ipc ──── state ──── window
  tray/ ────┘

  settings/ ── config/, pipe/, path
  window/  ── state, monitor（window/monitor.rs）
  monitor/ ── (独立，无业务依赖)
```

**约束**：
- 依赖单向，不允许反向依赖
- `config/` 不依赖任何业务模块（仅依赖 fontdb 等独立 crate）
- `state` 是可变状态的唯一所有者
- `pipe/`、`tray/` 不直接操作窗口，通过 `ipc` 间接通信
- **`ipc` 模块职责边界**：仅负责「主进程一侧接收设置进程的 reload / pos 请求（名称均附 session_id）」；托盘的菜单回调不经过 ipc，直接 spawn 子进程或触发主进程退出事件
- **`ipc` 仅主进程使用**：设置进程仅作为 pos-{session_id} 管道客户端（不需要 ipc 协调）；reload-{session_id} 管道由主进程监听。presence 由设置进程自身维护。实现上 `ipc.rs` 在主进程二进制中编译（与 `lyric.rs`、`window.rs`、`tray.rs` 同侧）

---

## 5. 数据流

### 5.0 主进程启动顺序（冷启动）

```
CreateMutexW(Local\lyric-for-musicfox-main) {
    if ERROR_ALREADY_EXISTS → exit(6) + stderr "another instance running\n"
    // 接管：持有 mutex 至进程退出
}
P0: config.load → std::thread::sleep(3s) → exit(0)
P1: eframe::run_native()
P3 前置于 GUI 启动：bind UDP receive_port { if EADDRINUSE → exit(1) }
P4（在已有 P3 前置 bind 后、GUI 启动前）：foreach (pipe_set in [reload, pos]) {
    CreateNamedPipeW(pipe_set); thread::spawn(ConnectNamedPipe)  // 名称均附当前 session_id
}
P3/P4: 完成各阶段前置步骤后进入 eframe::run_native()
```

**为什么先 Mutex 后管道**：`CreateMutexW` 是原子创建/打开操作；返回 `ERROR_ALREADY_EXISTS` 的实例不会进入管道创建。这样第二个实例不会创建额外 pipe server，也不会被 client 错误连接。只有取得新 mutex 的实例创建 reload / pos 服务端；服务端名称由 session_id 后缀区分会话。

### 5.1 运行时主循环

```
UDP 包 ─→ udp::recv (线程) ─→ state.update（Arc<RwLock<LyricState> 写锁）── 下一帧 GUI 主线程 update 读锁 ──→ window.render (vsync)
                                                       │
                                                       └→ scroll.update (per frame)

设置页变更 ─→ form.debounce ─→ config.save (.tmp → rename 成功)
                              │
                              ├→ pipe::reload.send（仅在 save 成功后）
                              └→ 等待主进程重载

主进程 reload-{session_id} 管道 ─→ ipc::on_reload (reload 监听线程)
                                       │
                                       ├→ config.load（同步 IO，**在 reload 线程完成**，不阻塞 GUI）
                                       └→ needs_reload AtomicBool = true → window.rebuild_style (GUI 主线程下一帧 vsync tick)

**reload-{session_id} 管道无重试**：设置进程仅发一次信号，不阻塞等待应答。原因：① reload 信号是「已落盘」的事后通知，不应假设主进程未响应就是失败；② 主进程忙时丢弃重建是 UI 选择的代价，不是数据丢失（config.toml 已落盘）。如需重新生效可让用户重启主进程（依赖 §13 验收项「config 落盘后重启生效」）。

主进程 pos-{session_id} 管道 ─→ ipc::on_pos_query ─→ state.get_pos

设置进程启动 ─→ pos_client.query ─→ pipe::pos.send GET_POS ─→ 接收 x,y ─→ form.set_pos（超时 1s 回退 config.toml 旧值）

设置进程启动 → Mutex 检查存在 → 连接 presence-{session_id} 管道 → 写入 ACTIVATE new_pid → 旧实例 presence 监听线程收到 → AllowSetForegroundWindow + FindWindowW + SetForegroundWindow + 应答 OK → 新实例读应答退出（超时 2s → exit code 5）

托盘左键 ─→ tray::on_left_click ─→ Command::new("lyric-for-musicfox --settings")
                                                              │
                                                              └→ 子进程自检单实例 → 激活或自启 → 父进程不感知
托盘右键 → tray::menu → 配置 / 退出
```

### 5.2 状态所有权

| 状态 | 所有者 | 读路径 | 写路径 |
|------|-------|-------|-------|
| `current_line.text` / `playing` | `state` (Arc<RwLock>) | window.render | udp::on_recv |
| `pos_x` / `pos_y` | `state` (Arc<RwLock>) | window.position, pipe::pos_answer | drag::on_release, monitor::on_change |
| 字体/颜色/样式 | `window.style` (局部) | window.render | ipc::on_reload |
| 配置 disk 镜像 | `config::Config` | settings.form | settings.on_save |

---

## 6. 并发模型

### 6.0 线程模型总则

**不使用 tokio**：所有 IO（UDP 接收、命名管道、托盘回调）使用 `std::thread::spawn` + 阻塞 IO + `std::sync` 同步原语。理由：
- IO 模式简单：UDP 包频率 ~10Hz，管道连接按需，tray 事件低频
- 同步原语足够：`Arc<RwLock>` + `Arc<AtomicBool>` + `crossbeam_channel` 覆盖全部场景
- 减少二进制体积：tokio 及其依赖会明显增加 exe 体积
- 调试直观：阻塞线程 + 同步原语比 async runtime 容易排查

**GUI 主线程不阻塞**：所有阻塞 IO 都在工作线程；GUI 主线程只渲染（`eframe::App::update`）+ 读内存状态（`Arc<RwLock<LyricState>>.read()`）。**例外**：设置进程保存按钮点击后的同步磁盘 IO（写 `.tmp` → atomic rename → reload 信号），在 GUI 线程同步执行。**理由**：保存是低频用户操作（手动点击），可接受几十 ms 闪烁；用 async 反而需要把保存逻辑抽到异步任务，复杂度上升与收益不成比例。如设入 P5 验收项：「保存动作主线程响应时间 ≤ 200 ms」。详见 §7.1.

### 6.1 线程清单

| 线程 | 职责 | 所属阶段 | 阻塞允许 |
|------|------|---------|---------|
| GUI 主线程 | eframe/winit 事件循环、歌词渲染、拖动 | P1+ | 否 |
| UDP 接收线程 | 阻塞 recv 16501 + JSON parse | P3 | 是（阻塞 IO） |
| Reload 管道监听线程 | ConnectNamedPipe 循环 | P4 | 是 |
| Pos 管道监听线程 | ConnectNamedPipe 循环（主进程一侧） | P4 | 是 |
| Presence 监听线程（设置进程） | 循环 ConnectNamedPipe 接收新实例激活请求；每请求：解析 ACTIVATE new_pid → AllowSetForegroundWindow + FindWindowW + SetForegroundWindow + 应答 OK → `FlushFileBuffers` + `DisconnectNamedPipe` + 丢弃句柄 + `CreateNamedPipe` 新实例等待下一个；Connect 返回 ERROR_PIPE_CONNECTED 也视为成功（req.md §4.2） | P4 | 是（串行处理；一次只处理一个请求） |
| 显示器监控线程 | 每 5s EnumDisplayMonitors（轮询兑底） + winit Window 过程 hook WM_DISPLAYCHANGE（事件驱动主路径） | P4（轮询 + hook 都在 P4 完成；P2 仅静态枚举） | 是（短时） |
| 日志 worker 线程 | tracing-appender flush | P0+ | 否（短时） |
| 设置进程窗口线程 | egui 事件循环 | P1+（表单元件）/ P4+（pos 拉取集成） | 否 |
| 设置进程 pos-{session_id} 管道客户端线程 | 拉取一次位置 | P4 | 是（短时） |

### 6.2 线程间通信

- **state** → `Arc<RwLock<LyricState>>`，GUI 主线程每帧读锁（vsync），UDP 线程写锁。**不加 channel**，UDP 包频率（~10Hz）远低于渲染频率（60-144Hz），加锁开销可忽略
- **style change** → `Arc<AtomicBool> needs_reload`，reload 线程置 true；GUI 主线程每帧检查后重建样式并重置标志。**不用 tokio::sync::Notify**（GUI 主线程不 await）
- **config save** → 设置进程同步串行（单线程，无需跨线程）
- **tray callback** → tray-icon 专用回调线程 → crossbeam_channel → GUI 主线程消费（避免在 tray 线程上 spawn 设置进程 / 调 ipc.send）

---

## 7. 关键设计决策

### 7.1 GUI 主线程不阻塞

所有可能阻塞 IO 的操作（UDP recv、管道 ConnectNamedPipe、文件读写）必须在独立线程。GUI 主线程仅：
- 处理 winit 事件（CursorMoved、MouseInput、Resize）
- 调用 egui 渲染
- 读 state + scroll 更新

### 7.2 状态可变源唯一

`state.rs` 是唯一可变状态。所有模块通过 `Arc<RwLock<State>>` / `Arc<AtomicBool>` / `crossbeam_channel`（仅 tray 回调）通信，避免散落的 `Mutex`。**UDP 走 RwLock，不用 channel**（§6.2 UDP 频率 ~10Hz < 渲染 60-144Hz，加锁开销可忽略）。

### 7.3 配置只由设置进程写

主进程只读 `config.toml`（启动一次 + reload 信号触发重读）。所有持久化都通过设置进程。
**原子性保证**：`rename` 由 Windows `MoveFileExW + MOVEFILE_REPLACE_EXISTING` 保证原子；主进程重读要么读到旧配置要么读到新配置，**不会读到中间状态**（避免 `rename` 进行中读到半截文件）。

### 7.4 激活握手（presence 管道，纯设置进程）

**设计动机**：Windows `SetForegroundWindow` 有前台锁限制，不同进程调用需旧实例主动 `AllowSetForegroundWindow(new_pid)` 授权。

**实现**：presence-{session_id} 管道仅由设置进程自维护，不依赖主进程。
- 第一个设置进程：CreateMutexW 通过 → 创建 presence-{session_id} 服务端 → ConnectNamedPipe 就绪
- 后续设置进程：连接 presence-{session_id} → 写入 `ACTIVATE ` + 新 PID + 换行 → 旧实例解析 → `AllowSetForegroundWindow` + `FindWindowW` + `SetForegroundWindow` + 应答 `OK`
- 超时 2s → 新实例 exit code 5

**为什么由设置进程自维护而非主进程**：
- 用户可能只启动设置进程（不启动主进程），主进程依赖会失败
- 激活的是设置窗口，与主进程无关

**为什么需要 presence 管道**：新实例自己调 `SetForegroundWindow` 会被 Windows 拒绝；必须由旧实例主动 `AllowSetForegroundWindow` 授权。

**该机制为 P4 阶段引入**；与 req.md §4.2 中描述的「激活已存在窗口」握手细节对应。

### 7.5 位置字段特殊

- 主进程内存独占：仅主进程启动时读 config.toml 一次（§10.1）；运行期间不重读
- 设置页通过 pos-{session_id} 管道拉取（启动时 1 次，1s 超时静默回退）
- reload-{session_id} 管道重载**不重读** pos_x/pos_y 文件值（即使 config.toml 中位置变更）
- 裁剪不写盘，等下次保存
- 拖动结果不自动落盘，需设置页保存
- 类型用 `i32` 全范围（不限制 16 位），覆盖 3 屏 4K 横排坐标
- **坐标单位**：**虚拟桌面物理像素**（不是 DIP）。req.md §7.2 坐标变换模型说明运行时 DIP ↔ 物理像素的转换点

### 7.6 崩溃恢复

- **设置进程崩了** → `.tmp` 保留（用户未保存编辑），下次启动优先用 `.tmp`
- **主进程崩了** → OS 回收 Mutex + UDP socket；托盘图标随进程销毁消失
- **解析失败** → 主进程默认配置启动 + 托盘闪烁；设置进程备份为 `.bak`

### 7.7 错误降级

- 日志写不进 → 降级 stderr
- 字体不存在 → 回退 Microsoft YaHei
- 管道信号丢失 → 接受 trade-off，依赖重启主进程

---

## 8. 数据结构（核心）

### 8.1 Config

**Codec 契约（P0 必须按此实现并单测）**：Runtime `Config` 保持严格类型并 `derive(Serialize, Deserialize)`；不用 RawConfig DTO。所有普通字段用**字段级** `#[serde(default = "default_x")]`，因为 struct 级 `#[serde(default)]` 不能保证单个缺失字段取得业务默认。`pos_x` / `pos_y` 和 `font_outline_color` 是唯一特殊字段：用成对的 `deserialize_with` / `serialize_with` 函数把 TOML 空字符串映射为 `None`。**不能手写 `impl Serialize for Config`**，避免与 derive 冲突。

```rust
use serde::{Deserialize, Deserializer, Serialize, Serializer};

fn default_width() -> u32 { 800 }
fn default_height() -> u32 { 80 }
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_pos() -> Option<i32> { None }
fn default_font_family() -> String { "Microsoft YaHei".into() }
fn default_font_size() -> f32 { 24.0 }
fn default_font_color() -> String { "#ffffff".into() }
fn default_outline_color() -> Option<String> { Some("#000000".into()) }
fn default_outline_width() -> u32 { 1 }
fn default_receive_port() -> u16 { 16501 }
fn default_send_port() -> u16 { 16502 }

// TOML 接受 pos_x = 123 或 pos_x = ""；其他字符串 / 类型是解析错误。
#[derive(Deserialize)]
#[serde(untagged)]
enum IntOrEmpty { Int(i32), Empty(String) }
fn de_pos<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i32>, D::Error> {
    match IntOrEmpty::deserialize(d)? {
        IntOrEmpty::Int(v) => Ok(Some(v)),
        IntOrEmpty::Empty(s) if s.is_empty() => Ok(None),
        IntOrEmpty::Empty(_) => Err(serde::de::Error::custom("position must be integer or empty string")),
    }
}
fn ser_pos<S: Serializer>(v: &Option<i32>, s: S) -> Result<S::Ok, S::Error> {
    match v { Some(v) => s.serialize_i32(*v), None => s.serialize_str("") }
}
fn de_outline<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let s = String::deserialize(d)?;
    Ok(if s.is_empty() { None } else { Some(s) })
}
fn ser_outline<S: Serializer>(v: &Option<String>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(v.as_deref().unwrap_or(""))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_window")]
    pub window: WindowConfig,
    #[serde(default = "default_style")]
    pub lyric_style: LyricStyleConfig,
    #[serde(default = "default_system")]
    pub system: SystemConfig,
}
fn default_window() -> WindowConfig { WindowConfig::default() }
fn default_style() -> LyricStyleConfig { LyricStyleConfig::default() }
fn default_system() -> SystemConfig { SystemConfig::default() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    #[serde(default = "default_width")] pub width: u32,
    #[serde(default = "default_height")] pub height: u32,
    #[serde(default = "default_pos", deserialize_with = "de_pos", serialize_with = "ser_pos")] pub pos_x: Option<i32>,
    #[serde(default = "default_pos", deserialize_with = "de_pos", serialize_with = "ser_pos")] pub pos_y: Option<i32>,
    #[serde(default = "default_true")] pub stay_on_top: bool,
    #[serde(default = "default_true")] pub frame_less: bool,
    #[serde(default = "default_false")] pub locked: bool,
}
impl Default for WindowConfig { fn default() -> Self { Self { width: default_width(), height: default_height(), pos_x: None, pos_y: None, stay_on_top: true, frame_less: true, locked: false } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricStyleConfig {
    #[serde(default = "default_font_family")] pub font_family: String,
    #[serde(default = "default_font_size")] pub font_size: f32,
    #[serde(default)] pub font_bold: bool,
    #[serde(default)] pub font_italic: bool,
    #[serde(default = "default_font_color")] pub font_color: String,
    #[serde(default = "default_outline_color", deserialize_with = "de_outline", serialize_with = "ser_outline")] pub font_outline_color: Option<String>,
    #[serde(default = "default_outline_width")] pub font_outline_width: u32,
}
impl Default for LyricStyleConfig { fn default() -> Self { Self { font_family: default_font_family(), font_size: default_font_size(), font_bold: false, font_italic: false, font_color: default_font_color(), font_outline_color: default_outline_color(), font_outline_width: default_outline_width() } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    #[serde(default = "default_receive_port")] pub receive_port: u16,
    #[serde(default = "default_send_port")] pub send_port: u16,
}
impl Default for SystemConfig { fn default() -> Self { Self { receive_port: default_receive_port(), send_port: default_send_port() } } }
impl Default for Config { fn default() -> Self { Self { window: WindowConfig::default(), lyric_style: LyricStyleConfig::default(), system: SystemConfig::default() } } }
```

**字段缺失与空串规则**：普通字段缺失取上表业务默认；普通字段显式空串为类型错误；`pos_x` / `pos_y` 缺失或空串均为 None；`font_outline_color` 缺失取 `Some("#000000")`，仅空串为 None。P0 单测必须覆盖：全缺失、每个特殊字段的缺失/空串/合法值/非法值，以及 `Config::default()` 序列化后再反序列化相等。

### 8.2 LyricState

```rust
#[derive(Debug, Clone, Default)]
pub struct LyricState {
    pub current_line: LineInfo,
    pub next_line: LineInfo,
    pub song_name: String,
    pub song_artist: String,
    pub playing: bool,
    pub time_ms: i64,                  // 仅日志，不入渲染
    pub pos_x: i32,                    // 运行时位置（Config::pos_x Option<i32> 启动时解析后写入；None → 主显示器居中计算为具体值；运行时裁剪/拖动后也是具体值）
    pub pos_y: i32,
}
// volume / mode 字段不进入 LyricState（仅反序列化供日志，不消费）

#[derive(Debug, Clone, Default)]
pub struct LineInfo {
    pub text: Arc<str>,                // Arc 避免每帧 copy
    pub words: Vec<LyricWord>,         // 仅存储不渲染
}

#[derive(Debug, Clone)]
pub struct LyricWord {
    pub word: String,
    pub start_time: i64,
    pub duration: i64,
}
```

### 8.3 UdpLyricPayload（外部协议）

```rust
// 字段级 default 函数（避免 struct 级 #[serde(default)] 不调用 impl Default 的陷阱）
fn default_type() -> String { "lyric_update".into() }
fn default_playing() -> bool { false }
fn default_time_ms() -> i64 { 0 }
fn default_volume() -> i32 { 0 }
fn default_mode() -> String { String::new() }
fn default_string() -> String { String::new() }
fn default_words() -> Vec<UdpWord> { Vec::new() }
fn default_i64() -> i64 { 0 }

#[derive(Debug, Deserialize)]
struct UdpLyricPayload {
    #[serde(rename = "type", default = "default_type")]
    type_: String,                     // 必须 == "lyric_update"，否则丢弃
    #[serde(default = "default_playing")]
    playing: bool,
    #[serde(default = "default_time_ms")]
    time_ms: i64,
    #[serde(default = "default_volume")]
    volume: i32,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "UdpSongInfo::default")]  // 嵌套 struct 缺失走字段类型 Default
    song: UdpSongInfo,
    #[serde(default = "UdpLineInfo::default")]
    current_line: UdpLineInfo,
    #[serde(default = "UdpLineInfo::default")]
    next_line: UdpLineInfo,
}

#[derive(Debug, Deserialize, Default)]
struct UdpSongInfo {
    #[serde(default = "default_string")]
    name: String,
    #[serde(default = "default_string")]
    artist: String,
}

#[derive(Debug, Deserialize, Default)]
struct UdpLineInfo {
    #[serde(default = "default_string")]
    text: String,
    #[serde(default = "default_words")]
    words: Vec<UdpWord>,
}

#[derive(Debug, Deserialize)]
struct UdpWord {
    #[serde(default = "default_string")]
    word: String,
    #[serde(default = "default_i64")]
    start_time: i64,
    #[serde(default = "default_i64")]
    duration: i64,
}
```

**字段缺失走字段级 default 函数**：与 Config 的字段级 default 策略一致。**UDP 字段全是基本类型**（String / i64 / bool），不需要配置文件的特殊 Option 空串 codec。

// ... 详细见 req.md §6.2

---

## 9. 错误处理策略

| 层级 | 策略 |
|------|------|
| 模块内部 | `Result<T, E>` 向上传播，不静默吞错 |
| 边界（UI/IO） | 降级 + 日志 warn，不 panic |
| 启动期 | 关键错误 → stderr + exit code 非零（见下表） |
| 运行时 | UDP 丢包/管道信号丢失 → 接受 trade-off，记 warn |

### UDP 包接收策略

- 单包大小限制 8 KB（recv buffer 上限）：超过直接丢弃 + warn 日志
- `type` ≠ `lyric_update`：丢弃 + warn
- JSON parse 失败：丢弃 + warn
- 字段缺失：走 §req.md 12.1 字段缺失策略（Default::default()）

### Exit code 字典

| Exit code | 场景 |
|-----------|------|
| 0 | 正常退出 |
| 1 | 端口被不明程序占用（Mutex 通过 + bind 失败） |
| 2 | 配置路径不可写（%APPDATA% 不存在或权限不足） |
| 3 | Mutex 创建失败（Windows API 异常） |
| 4 | 参数错误（CLI 解析失败 / 未知参数）；**P0 的 `--settings`**（该模式从 P1 才可用） |
| 5 | 设置进程激活旧实例失败（FindWindowW / AllowSetForegroundWindow / SetForegroundWindow 任一失败） |
| 6 | **主进程** Mutex 已存在（认定本项目主进程实例已在运行），该实例静默退出 + stderr "another instance running\n"。设计选择：「不抢焦点」语义优先于「告知用户」。**设置进程不走 exit 6**——它有 presence 路径激活旧实例后 exit 0，或激活失败后 exit 5 |

---

## 10. 测试策略

每个阶段结束跑：

1. `cargo build --release` 通过
2. 阶段对应的手工验收清单（见 §2）
3. `cargo clippy -- -W clippy::correctness` 通过（不阻断风格类警告，避免与上游 crate 冲突）
4. **单元测试覆盖**：
   - `config`：TOML 序列化/反序列化、字段缺失策略、字段类型错误
   - `validate`：颜色正则、范围、字体存在
   - `scroll`：offset_x 状态机、文本变更重置、循环判定
   - `udp.rs`：JSON 协议解析（输入字符串 → 输出 UdpLyricPayload，含 type 校验、字段缺失、类型错误场景）
   - `pipe.rs`：pos-{session_id} 管道协议（GET_POS 请求 → 应答；None 编码为 EMPTY,EMPTY；超时）
   - `command`：6 个出站命令序列化
5. **集成测试**（仅 P5）：
   - 启动 mock UDP 发送方 → 启动主进程 → 验证窗口渲染（image snapshot 或日志断言）
   - 主+设进程交互：设置保存 → 验证主进程 reload 收到 + 样式重建

---

## 11. 风险与开放问题

| 项 | 风险 | 缓解 |
|----|------|------|
| `eframe` 与 `winit` API 频繁变动 | 编译失败或运行时行为变化 | 锁版本到具体 minor（`winit = "0.30"`），build.rs 显式声明 |
| `tray-icon` 闪烁实现（双 icon 切换） | 1Hz 切换在某些 Windows 版本上有抖动 | 实测验证，必要时降到 0.5Hz |
| `WS_EX_TRANSPARENT` 与 `with_active(false)` 冲突 | 可能一个生效另一个失效 | 正交不冲突：WS_EX_TRANSPARENT 控 hit-test 透明；WS_EX_NOACTIVATE / with_active(false) 控不抢焦点。两者可同时设置。P2 实测验证 |
| `egui` set_fonts 是否真的不重建窗口 | 文档已标"实现期验证" | P2 实测验证步骤：(1) 启动后记录当前 frame 时间 (2) 手动触发 set_fonts (3) 记录 frame 时间 (4) 对比中断时长；接受 ≤ 50ms |
| UDP 接收线程 panic | 影响 lyric 更新 | `std::thread::spawn` + 主循环 `std::panic::catch_unwind(AssertUnwindSafe(|| recv()))`，panic 时重启线程并记 error 日志。**不用 tokio**（UDP 是阻塞 IO） |
| Windows Mutex 跨用户会话冲突 | Local\ 命名空间 = 会话级；同一会话内同名字才冲突 | 不同 RDP 会话互不影响（各自 Local 命名空间隔离）。**MVP 不需要处理多会话冲突**，该风险项删除 |
| tray-icon 高 DPI 图标清晰度 | 双 .ico 资源在不同 DPI 下表现不同 | assets/icon.ico 与 icon_gray.ico 均提供多分辨率（16/32/48/256）；运行时由 tray-icon 选择最匹配 |

---

## 12. 验收进度跟踪

- [x] P0 骨架
- [x] P1 歌词窗口
- [x] P2 拖动 + 锁定 + 副屏
- [x] P3 UDP 通信
- [ ] P4 托盘 + 进程协作
- [ ] P5 出站 + 收尾

**实现笔记责任**：每个 P 阶段由主开发者写 **200-500 字**实现笔记，追加到 `docs/IMPLEMENTATION_NOTES.md`，包含：(1) 阶段内关键决策 (2) 与 req.md / dev.md 偏差及原因 (3) 下阶段依赖项 / 遗留问题。

**与 req.md §15 关系**：
- dev.md §12 跟踪**阶段完成**（项目里程碑）
- req.md §15 跟踪**功能点验证**（跨阶段汇总的验收清单）
- 两份独立但互补；阶段勾选 ≠ 所有对应功能点都验证通过
