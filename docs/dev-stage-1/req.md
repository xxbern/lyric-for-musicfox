# lyric-for-musicfox 需求文档

> 版本：0.2.0（重构版）
> 平台：Windows 10 / 11 (x86_64)
> 语言 / 框架：Rust + egui + eframe

---

## 1. 项目背景

`lyric-for-musicfox` 是 **[go-musicfox](https://github.com/go-musicfox/go-musicfox)** 桌面播放器的**歌词桌面扩展**。

- go-musicfox 是基于 Go 的命令行音乐播放器，本身不提供桌面 UI。
- 本项目作为独立 Windows 进程运行，通过 **UDP** 接收 go-musicfox 推送的实时歌词与播放状态，并以**透明顶层窗口**的形式渲染歌词。
- 通过反向 UDP 命令，用户可以控制 go-musicfox 的播放、暂停、切歌、跳转、音量等行为。

```
┌──────────────────┐    UDP 推送 (入站)    ┌──────────────────────┐
│   go-musicfox    │ ───────────────────▶  │   lyric-for-musicfox    │
│   (播放器)        │ ◀─────────────────── │   (歌词窗口 + 托盘)   │
│                  │    UDP 命令 (出站)    │                      │
└──────────────────┘                       └──────────────────────┘
```

> 本项目不包含音乐解码、不修改音频，仅消费歌词数据并展示。

---

## 2. 核心功能

| # | 功能 | 说明 |
|---|------|------|
| F1 | 歌词显示 | 渲染 UDP 推送的当前歌词文本（go-musicfox 端已预解析对齐） |
| F2 | 透明叠加 | 透明无边框顶层窗口，不抢焦点 |
| F3 | 自动横向滚动 | 窗口宽度 < 歌词宽度时自动横向滚动 |
| F4 | 占位提示 | 启动后未收到数据时显示低透明度占位符 |
| F5 | 拖动定位 | 鼠标拖动调整窗口位置（可锁定） |
| F6 | 样式自定义 | 字体、字号、颜色、加粗、斜体、描边 |
| F7 | 系统托盘 | 常驻托盘，左键打开设置、右键菜单 |
| F8 | 独立设置进程 | `--settings` 启动独立的设置 GUI |
| F9 | 单实例 | 主进程、设置进程各自唯一 |

---

## 3. 用户场景

1. 用户双击运行 `lyric-for-musicfox.exe`，出现**透明顶层歌词窗口**，初始位置居中，初始文本为占位符 `"......"`。
2. go-musicfox 启动并播放音乐后，UDP 数据到达，歌词实时更新为当前行。
3. 当前行文本**比窗口宽度宽**时，文本自动横向循环滚动。
4. 用户用鼠标拖动歌词窗口到屏幕任意位置。
5. 用户**右键托盘图标**弹出菜单，点击"配置" → 启动设置窗口（若已运行则聚焦）。
6. 用户在设置窗口修改字体、颜色、位置等 → 点击"保存"。
7. 保存后 500ms 内歌词窗口以新配置重新渲染。
8. 用户**右键托盘图标 → 退出**，主进程关闭，歌词窗口销毁。

---

## 4. 进程模型

### 4.1 双进程

| 进程 | 启动方式 | 职责 |
|------|---------|------|
| **主进程 (lyric)** | `lyric-for-musicfox.exe`（无参数） | UDP 监听、歌词渲染、系统托盘 |
| **设置进程 (settings)** | `lyric-for-musicfox.exe --settings` | 设置 GUI、读写配置文件 |

### 4.2 单实例

| 进程 | 单实例机制 |
|------|-----------|
| 主进程 | **启动顺序**：① CreateMutexW `Local\\lyric-for-musicfox-main`（Local 命名空间 = 用户会话级）→ ② Mutex 已存在 → 当前进程静默 exit 6 + stderr `another instance running\n`（认定已有实例），**不创建任何管道/socket**；③ Mutex 不存在 → 接管：创建命名管道 reload/pos 两个服务端（名称均含当前 session_id）并 ConnectNamedPipe 就绪 → bind UDP `0.0.0.0:16501` → ④ bind 失败 → 报错退出（别的不明程序占了 16501）。Mutex 句柄在进程生命周期内持有，退出时由 Drop 自动 ReleaseMutex。**两层语义**：Mutex 检测「本项目实例是否已运行」；UDP bind 端口检测「是否有其他进程占用本项目预期端口」。两个检测是互补而非重复：Mutex 不存在 + bind 成功 → 本项目首次启动；Mutex 不存在 + bind 失败 → 别的不明程序占用 16501；Mutex 存在 → 本项目已运行，无需 bind（直接退出）。**启动顺序要点**：先 Mutex 判定 + 退出 不必要的进程，避免第二个实例创建多余的管道/socket、避免 client 错误连接到即将退出的实例。
**可达性确认**：UDP 无 TIME_WAIT 状态，进程崩溃后 OS 立即回收 socket 资源；Mutex 句柄由 OS 在持有进程崩溃时自动释放。因此本项目崩溃后不会有「Mutex 已释放但 UDP socket 仍被占用」的中间态。 |
| 设置进程 | **Windows Mutex** 命名为 `Local\\lyric-for-musicfox-settings`。已有实例时：① 激活已存在窗口到前台（BringWindowToTop / SetForegroundWindow），② 当前进程退出。 |

**前台锁绕过**：Windows 限制 SetForegroundWindow 跨进程使用（防骚扰）。握手流程走 **presence 管道**（纯设置进程间，不依赖主进程）：旧实例 presence-{session_id} 监听线程收到新实例的 `ACTIVATE <new_pid>` 后调 AllowSetForegroundWindow(new_pid) + FindWindowW + SetForegroundWindow 激活；新实例读 `OK` 应答退出。详见 §4.2「AllowSetForegroundWindow 握手」。

**设置进程启动顺序**（区分两种情况）：
- **情况 A：Mutex 不存在（当前用户是第一个设置进程）** → 创建 presence-{session_id} 管道服务端（用于被后续设置实例激活）→ 走正常配置流程：初始化 config.toml + .tmp → 渲染设置 UI。
- **情况 B：Mutex 已存在（已有设置实例运行）** → ① 连接旧实例的 presence-{session_id} 管道 → ② 写入 `ACTIVATE ` + 新实例 PID + 换行 → ③ 旧实例 presence 监听线程收到后调 `AllowSetForegroundWindow(new_pid)` 授权 + `FindWindowW` 找旧设置窗口（标题 `lyric-for-musicfox - 设置`）+ `SetForegroundWindow` 激活 → ④ 应答 `OK` + 换行 → ⑤ 新实例读应答退出（超时 2s）。**任一步失败 → exit code 5**。

**并发与可靠性**：
- **会话化管道名**：presence 名称为 `\\.\pipe\lyric-for-musicfox-presence-{session_id}`；`session_id` 由 `ProcessIdToSessionId(GetCurrentProcessId())` 获得十进制值。只有同一 Windows 会话的设置进程会使用相同 pipe 名，不能依赖 `Local\` Mutex 去隔离全局 pipe 名。
- **服务端实例**：旧实例一次创建一个服务端。每轮先 `CreateNamedPipeW`，再 `ConnectNamedPipe`；`ConnectNamedPipe` 返回 false 且 `GetLastError()==ERROR_PIPE_CONNECTED` 也视为连接成功。请求完成后必须 `FlushFileBuffers` → `DisconnectNamedPipe` → 丢弃句柄，随后创建一个新实例等待下一请求。
- **客户端连接**：新实例在总计 2s deadline 内循环：`CreateFileW` 失败且错误为 `ERROR_PIPE_BUSY` / `ERROR_FILE_NOT_FOUND` → `WaitNamedPipeW(remaining_ms)` 后重试；其他错误立即 exit 5。连接成功后写完整请求并读完整 `OK\n`；超时或非 OK 应答 exit 5。
- **并发语义**：请求串行；多个新实例可在 deadline 内排队，任一个激活成功均允许其 exit 0。

**该机制不依赖主进程**：presence 管道由设置进程自维护；用户仅启动设置进程（不启动主进程）时仍可正常激活已有设置窗口。

**Mutex 检测是单实例判定唯一机制**；FindWindowW 仅用于在已知 Mutex 已存在的前提下定位旧窗口句柄。

**管道 IO 模式与 ACL**：使用**阻塞 IO**（不用 FILE_FLAG_OVERLAPPED），简化代码；ACL 使用默认 DACL（以实际 Windows token / 用户 ACL 为准，P4 人工验证同用户可连、不同用户被拒）。reload / pos 是主进程单会话服务：名称也必须附 `-{session_id}`，session_id 与 presence 的获取方式相同。pid 管道已移除。

### 4.3 进程间数据流

```
            ┌──────────────────────────────────────────────────┐
            │   %APPDATA%/lyric-for-musicfox/                       │
            │       config.toml          （正式配置）            │
            │       config.toml.tmp      （设置 GUI 工作副本）   │
            └──────────────────────────────────────────────────┘
                              ▲                       │
                       读取   │                       │ 写入（保存按钮）
                              │                       ▼
                ┌─────────────┴─────────────┐  ┌──────────────────┐
                │   主进程                   │  │   设置进程        │
                │  - 启动时读 config.toml    │  │  - 启动时拷贝       │
                │  - 拖动仅更新内存 pos      │  │    config.toml    │
                │  - 收到"重载"信号          │  │    → config.toml.tmp │
                │    重新加载并重建样式       │  │  - 表单编辑 .tmp   │
                │  - **不直接写 config.toml**│  │  - 保存时原子写回   │
                └───────────────────────────┘  └──────────────────┘
```

设置进程在保存时：
1. 校验所有字段合法性（见 §10）。
2. 校验通过 → 原子写 `%APPDATA%/lyric-for-musicfox/config.toml`（先写 `.tmp` 再 rename）。
3. 通过命名管道 `\\.\pipe\lyric-for-musicfox-reload-{session_id}` 通知主进程：配置已变更，请重载。
4. 主进程重载后立即以新配置重新渲染歌词窗口。

#### 进程间管道汇总

| 管道名 | 方向 | 触发时机 | 负载 | 主进程未启动时行为 |
|--------|------|---------|------|-------------------|
| `\\.\pipe\lyric-for-musicfox-pos-{session_id}` | 双向（设置问、主进程答） | 设置进程启动拉取一次当前窗口位置 | 请求 "GET_POS\n" → 应答 "x,y\n"（一行逗号分隔整数），`pos_x` 为 `None` 时编码为 `EMPTY,EMPTY` | 设置进程 1s 超时后静默回退到 `config.toml` 旧值，不弹窗、不提示用户（位置字段由主进程独占，「位置可能是旧的」是已知预期）。**每次拉取建立新连接、单次请求单次应答、然后双方关闭** |
| `\\.\pipe\lyric-for-musicfox-reload-{session_id}` | 单向（设置写、主进程读） | 设置进程保存按钮点击后 | 任意 1 字节 | 信号丢失，依赖重启主进程兜底（见下）；**主进程未运行时**：设置进程 connect 管道会立即失败 → 信号丢弃，config.toml 仍已落盘，下次启动主进程生效 |
| `\\.\pipe\lyric-for-musicfox-presence-{session_id}` | 双向（新写、旧读+应答） | 设置进程间自激活握手 | 请求 `ACTIVATE <new_pid>\n` → 应答 `OK\n` | 仅设置进程使用（**不依赖主进程**）；旧实例循环监听；客户端总 deadline 2s，超时 → exit code 5 |

#### AllowSetForegroundWindow 握手（presence 管道，由设置进程自维护）

Windows 限制不同进程间 `SetForegroundWindow` 的使用（防骚扰前台锁）。本项目用 **presence-{session_id} 管道** 完成握手，**仅由设置进程维护，不依赖主进程**。

**完整握手流程**（纯设置进程间）：
1. 第一个设置进程启动 → CreateMutexW 通过 → 创建 presence-{session_id} 管道服务端 → ConnectNamedPipe 就绪 → 渲染设置 UI
2. 后续设置进程启动 → CreateMutexW 检测已存在
3. → 连接 presence-{session_id} 管道（旧实例服务端） → 写入 `ACTIVATE <new_pid>` + 换行
4. 旧实例 ConnectNamedPipe 接收 → 解析 new_pid → `AllowSetForegroundWindow(new_pid)` 授权 → `FindWindowW` 找旧设置窗口 → `SetForegroundWindow` 激活 → 应答 `OK` + 换行
5. 新实例读应答退出（exit code 0）；超时 2s → exit code 5

**为什么由设置进程自维护而非主进程**：
- 用户可能只启动设置进程（不启动主进程），主进程依赖会失败
- 激活的是设置窗口，与主进程无关
- 主进程只维护自己的 reload / pos 两个管道（详见 §4.3 表格）

**为什么需要 presence-{session_id} 管道**：新实例自己调 `SetForegroundWindow` 会被 Windows 拒绝；必须由旧实例主动 `AllowSetForegroundWindow` 授权后才能激活。

> **主进程不轮询配置文件变更**，仅依赖设置进程的显式通知。
>
> **通知丢失的处理**：若命名管道信号丢失（极端场景：管道缓冲区满、主进程涰起中、权限问题等），设置进程的保存操作仍会成功（`config.toml` 已落盘），但主进程不会重建样式。此时用户可见现象为「保存后样式未变化」。
>
> 兑底手段：重启主进程即可读到最新样式。**不加心跳 / 重试** —— 设置保存是低频用户主动操作，丢失概率极低，重试逻辑会增加复杂度但收益有限。
>
> **进程归属**：
> - **reload / pos 两个管道**：仅主进程维护；管道名附当前 session_id。reload/pos 服务端在主进程；设置进程仅作为 pos 客户端（启动时拉取位置）。主进程一侧包含 `ipc` 协调模块，设置进程不包含 `ipc`。
> - **presence-{session_id} 管道**：仅设置进程维护。设置进程既作为服务端（被后续设置实例激活）也作为客户端（激活已有实例）。主进程不参与该管道。

---

## 5. 命令行接口

```bash
# 启动歌词主进程
lyric-for-musicfox.exe

# 启动设置独立进程（P1 起可用；P0 返回 exit code 4）
lyric-for-musicfox.exe --settings

# 显示版本
lyric-for-musicfox.exe --version

# 显示帮助
lyric-for-musicfox.exe --help
lyric-for-musicfox.exe --benchmark    # 开发期输出 UDP→渲染延迟统计到 stderr，不影响正常运行（仅增加 trace 日志输出）
```

---

## 6. UDP 通信协议

### 6.1 端口

| 方向 | 绑定地址 | 端口 | 说明 |
|------|---------|------|------|
| 接收（入站） | `0.0.0.0:16501` | 16501 | 接收 go-musicfox 推送的歌词 |
| 发送（出站） | `127.0.0.1:16502` | 16502 | 向 go-musicfox 发送控制命令 |

### 6.2 入站：歌词负载

消息体为 JSON，反序列化结构如下：

```rust
struct UdpLyricPayload {
    r#type:    String,            // 固定值 "lyric_update"
    playing:   bool,
    time_ms:   i64,               // 当前播放位置（毫秒）
    volume:    i32,               // 0-100
    mode:      String,            // 播放模式描述（预留字段，当前版本不渲染也不控制，预留扩展如单曲循环 / 随机 / 顺序等状态展示）
    song:      UdpSongInfo,
    current_line: UdpLineInfo,
    next_line:    UdpLineInfo,
}

struct UdpSongInfo {
    id:          i64,
    name:        String,
    artist:      String,
    album:       String,
    pic_url:     String,
    duration:    i64,             // 总时长（毫秒）
    is_favorite: bool,
}

struct UdpLineInfo {
    text:  String,
    words: Vec<UdpLyricWord>,     // 当前版本仅存储不渲染（预留卡拉 OK）
}

struct UdpLyricWord {
    word:       String,
    start_time: i64,              // 毫秒
    duration:   i64,              // 毫秒
}
```

> **入站数据合并策略**：UDP 数据到达时，仅更新以下字段到主进程内存状态：
> - `current_line.text` ← 渲染管线消费
> - `next_line.text` ← **仅存储，渲染不消费**（预留如预加载、过渡动画）
> - `song.name` / `song.artist` ← **仅存储，渲染不消费**（预留如元信息展示）
> - `playing` ← 渲染管线消费（控制 alpha）
> - `time_ms` ← 仅反序列化供 trace 日志，渲染不消费
>
> 其余字段保留以备未来扩展（不参与当前 UI 渲染）。
>
> **LRC 处理边界**：本项目不解析 LRC 文本。go-musicfox 端负责：解析原始 LRC、根据 `time_ms` 查找当前行 / 下一行、推送给本项目。本项目仅消费预解析结果，多行 LRC 的取舍（哪行作 current、哪行作 next）由 go-musicfox 决定。
>
> **`words` 字段**：当前版本仅反序列化、不参与渲染。保留字段是为了协议向前兼容（未来需要卡拉 OK 效果时无需改动 schema）。
>
> **`time_ms` 字段**：仅反序列化供日志/调试输出（trace 级别），本版本渲染管线不消费（无进度条、无卡拉 OK）。切行时仅依赖 `current_line.text` 是否变更；若 `time_ms` 与 `current_line.text` 表达的进度不一致，以 `current_line.text` 为准。

### 6.3 出站：控制命令

```rust
struct UdpCommand {
    command: String,              // 见下表
    value:   Option<i64>,         // seek/volume 需要
}
```

| 命令 | `value` | 说明 |
|------|---------|------|
| `toggle` | `None` | 播放 / 暂停切换 |
| `next` | `None` | 下一首 |
| `previous` | `None` | 上一首 |
| `like` | `None` | 收藏 / 取消收藏 |
| `seek` | `Some(毫秒)` | 跳转到指定位置 |
| `volume` | `Some(0-100)` | 设置音量 |

发送目标固定 `127.0.0.1:16502`；**发送方本地端口由 OS 临时分配**，不绑定固定端口（避免与 go-musicfox 或其他进程的发送端口冲突）。失败仅记录日志，不重试、不弹窗。

**本版本不出站 UI 触发**：§3 §9 §10 均未提供出站命令的 UI 触发入口。下方表格列出的是**协议完整性定义 + 内部 API**，P5 阶段仅完成 `Command` enum 封装 + UDP 发送实现；**不代表本版本可被用户调用**。未来扩展预留。

---

## 7. 歌词叠加窗口

### 7.1 窗口特性

| 特性 | 值 |
|------|---|
| 装饰 | 无边框 (`frame_less = true`) |
| 内容 | **仅歌词文本**：窗口里没有标题栏、背景框、菜单、按钮、滚动条等任何 UI 元素；除歌词文本外的所有像素 alpha = 0（完全透明） |
| 层级 | Always-on-top |
| 焦点（点击穿透） | 按 `locked` 状态切换，详见下表 |
| 可调整大小 | 否（仅拖动位置） |
| 关闭按钮 | 无；通过托盘菜单退出 |

#### 点击穿透行为

| `locked` | 鼠标左键行为 | 点击穿透 | 视觉效果 |
|---------|------------|---------|---------|
| `true`  | **完全无响应**（既不拖动也不抢焦点） | ✅ 开启 | 鼠标在歌词窗口上按下，事件直接穿透到下层窗口，歌词窗口对用户"不存在" |
| `false` | 按下+移动可拖动窗口 | ❌ 临时关闭 | 拖动期间窗口接收鼠标事件；松开后**窗口仍是正常 hit-test 状态**（locked = false 本就没有 WS_EX_TRANSPARENT），**不主动抢焦点**（拖动期间也不获取焦点，松开后焦点保持在下层窗口） |

> **设计意图**：`locked = true` 时用户已明确表示"位置固定"，此时歌词窗口应当**对鼠标完全透明**——用户根本感知不到这层窗口存在，所有点击直达下层应用（如浏览器、游戏全屏、视频播放器），不造成任何遮挡干扰。

> **"整个窗口 = 一个透明画布 + 一行歌词文本"**。`locked = false` 时，鼠标在窗口任意像素（包括歌词文字本身、空白像素）上按住左键都能触发拖动，行为完全一致。
>
> **实现约束**：eframe 默认 hit-test 仅命中文字 + 背景框；本项目必须**整窗口 hit-test**（即 alpha = 0 的透明像素也算"在窗口内"）。Windows 实现方式：
> - **locked = true**：仅此时注入 `WS_EX_TRANSPARENT` 扩展样式（SetWindowLongPtrW + GWL_EXSTYLE），整窗口 hit-test = false，事件穿透到下层应用。这是「完全无响应 + 点击穿透」的可靠实现。
> - **locked = false**：窗口默认就是正常 hit-test（**不需要移除任何样式**，窗口本身就没有 WS_EX_TRANSPARENT）；eframe/egui 的 hit-test 范围默认是整个矩形区域（只要 alpha > 0 就算），本项目设置 alpha = 0 但因为是「整窗口 hit-test」所以仍接收鼠标事件。
> - **拖动期间不获取焦点**：创建 winit Window 时使用 `with_active(false)` 选项，拖动过程窗口不进入 active 状态，下层窗口焦点不被抢。**拖动后不需「恢复穿透」**——locked = false 时窗口本来就只是不抢焦点，不是 WS_EX_TRANSPARENT。
> - **API 选型**：eframe/egui 没有公开 API 操作 Windows 扩展样式，需通过 `windows` crate 在 winit Window 创建后调 `SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ...)` 注入；本项目不使用 `Window::set_cursor_hittest`（那是 cursor icon API，与 hit-test 无关）。

### 7.2 坐标与尺寸（位置字段 = 虚拟桌面物理像素，尺寸字段 = DIP）

> **统一事实源**：**位置字段**（pos_x / pos_y）以**虚拟桌面物理像素**为唯一事实源（跨屏连续、与显示器物理布局对应、Windows API 原生单位）。
> **尺寸字段**（width / height / 字号 / 描边宽度）以 **DIP** 为单位（egui 默认单位，跟随 DPI 缩放）。
> 运行期唯一转换点：渲染时调用 `pixels_per_point()` 获取当前显示器 DPI 用于 egui 缩放；位置字段不参与转换。

| 字段 | 默认值 | 单位 | 说明 |
|------|--------|------|------|
| `width` | 800 | DIP | 窗口宽度 |
| `height` | 80 | DIP | 窗口高度 |
| `pos_x` | `None` → **启动时主显示器工作区**水平居中 | 虚拟桌面物理像素 | 全屏**虚拟桌面**绝对坐标 X（支持副屏定位，见下） |
| `pos_y` | `None` → 主显示器工作区顶部偏下 100 DIP 转物理像素 | 虚拟桌面物理像素 | 全屏**虚拟桌面**绝对坐标 Y |

**None 语义**：`pos_x = None` 表示"未指定位置"，**仅在进程启动时**根据主显示器工作区计算坐标（物理像素）；启动后 None 与 Some 值行为一致（都是具体物理像素坐标）。用户拖动后 None 会立即变成 Some（拖动结果必然是具体物理像素坐标）。

**默认位置基准**：使用 Windows 工作区（work area / usable area），即**减去任务栏后的可用矩形**（物理像素）。避免默认位置被任务栏遮挡。计算公式：`pos_x = (主显示器工作区宽度 - window_width_physical_px) / 2`；`pos_y = 主显示器工作区 top`。

#### 副屏定位

- 坐标系为 Windows **虚拟桌面**（virtual desktop），原点为**主显示器左上角**，跨屏连续。
  - 主屏 1920×1080（100%），副屏位于主屏右侧 1920×1080（150%） → 副屏左上角为物理像素 (1920, 0)，右下角为 (4800, 1620)。**副屏物理像素为 2880×1620**（150% 缩放使同样逻辑区域占更多物理像素）。
  - 若副屏在主屏左侧，则坐标为负。
- 拖动到任意屏幕任意位置 → 记录**虚拟桌面物理像素**，重启后窗口精确还原到该位置。
- 主显示器变更 / 副屏拔除 → 启动时按 §14 边界处理（裁剪到新主显示器可见区）。
- **多显示器不同 DPI 缩放**：本项目**仅持久化物理像素**。跨 DPI 屏幕的拖动和定位结果一致（不依赖任何 DPI 转换）。

#### 坐标变换模型（P2 阶段实现）

**单一事实源**：**虚拟桌面物理像素**（Windows API 默认单位）。**不存 DIP**。

理由：
- 虚拟桌面是 Windows 管理的**连续**物理像素坐标系，跨屏边界连续（不管 DPI）
- 持久化跨 DPI 副屏时，物理像素是唯一**与显示器物理布局对应**的坐标
- DIP 依赖单个显示器的 DPI，跨屏边界会发生跃变（150% 屏的 100 DIP = 150 物理像素；100% 屏的 100 DIP = 100 物理像素；同一 DIP 在两屏不同物理位置）
- Windows API 不提供「全局 DIP」——DIP 是 per-monitor 的相对概念，不是连续坐标系

**输入 / 输出 / 转换点**：

| 阶段 | 单位 | 转换 |
|------|------|------|
| 配置文件磁盘存储 | 虚拟桌面物理像素 | （无需转换） |
| winit `WindowEvent::CursorMoved` / `Resized` | 物理像素 | 本就是虚拟桌面物理像素，直接使用 |
| `Window::set_outer_position()` 参数 | 物理像素 | 本就是虚拟桌面物理像素，直接使用 |
| 保存到内存 `state.pos_x` / `LyricState.pos_x` | 虚拟桌面物理像素 | （运行期唯一事实源） |
| **渲染中文字布局 / 字体度量** | DIP | 取**该文字位置所在显示器**的 DPI，转 DIP 用于字体度量计算 |
| **拖动中实时计算** | 物理像素 | 拖动过程本就是物理像素，不需转换 |

**跨 DPI 场景**：主屏 100%、副屏 150% 缩放；窗口从主屏拖到副屏 → 同一物理像素位置。**持久化存物理像素**，跨屏位置保持一致。**不进行跨屏 DPI 重新映射**。重启后窗口位置与拖动时完全一致。

**边界场景**：虚拟桌面物理像素可能超过 32767（多屏 4K 横排）→ **不限制**（`pos_x` 是 `i32`，可表达 21 亿）。虚拟桌面起点可能为负（副屏在主屏左侧）→ `i32` 表达负值无问题。

**为什么不是 DIP**：DIP 在 Windows 跨屏虚拟桌面中**不是一个明确定义的全局连续坐标系**。本项目使用「虚拟桌面物理像素」作为唯一事实源，仅在字体度量 / 布局阶段转 DIP。

**补充**：req.md §8 中 `width = 800`、`height = 80` 等尺寸字段仍以 **DIP** 为单位（这是 egui 默认单位）；只在位置字段（pos_x/pos_y）使用物理像素。运行时绘制通过 `pixels_per_point()` 获取当前显示器 DPI 用于 egui 缩放。

### 7.3 文本渲染规则

| 情况 | 行为 |
|------|------|
| 内存中 `current_line.text` 为空字符串 / 进程刚启动 / UDP 包未到达 | 显示占位符 `"......"`，alpha = 0.3 |
| 文本宽度 ≤ 窗口宽度 | 居中显示，单行 |
| 文本宽度 > 窗口宽度 | **自动横向滚动**：从右向左匀速循环，滚动速度固定（30 DIP/秒） |
| `playing = true` | 文本 alpha = 1.0 |
| `playing = false` | 文本 alpha = 0.5，**滚动继续**（仅透明度变化，不打断动画） |

> §7.3 中所有状态判断（占位 / 滚动 / 透明度）均依赖 `current_line.text` 与 `playing`，**不使用 `time_ms`**。`time_ms` 仅在 trace 日志中输出。

#### 渲染驱动与时钟

| 驱动源 | 负责 | 频率 |
|--------|------|------|
| **vsync（egui 默认）** | 窗口重绘 / 滚动动画推进 / 占位符渲染 | 跟随显示器刷新率（60/120/144Hz） |
| **UDP 数据到达** | 内存中 `current_line.text` / `next_line.text` / `playing` 更新 | 数据驱动，不触发重绘 |

**两条路径独立**：滚动动画完全在 vsync tick 内按 `delta_time` 推进；UDP 更新仅改内存，下次 vsync tick 自然读到新值。UDP 包频率不影响滚动流畅度，丢包不影响动画连续性。

#### 滚动循环模式（marquee）

文本从窗口**右边缘外**（向右偏移 = 文本宽度）匀速向左移动；当文本尾端正好离开左边缘时，下一轮从右边缘外重新进入；保持匀速 30 DIP/秒，**无停顿、无回弹、无淡入淡出**。视觉上像跑马灯无缝循环。

**滚动粒度**：**像素级**（按 DIP 偏移量推进），不是字符级。中英文混排 / 不同字宽字符都能平滑滚动；不会出现"中文走一个字、英文走半个字"的步进不一致。

**滚动与 vsync 的关系**：滚动偏移量是**浮点累积**（`offset_x += 30.0 * delta_time`），与 vsync 频率解耦。60Hz、120Hz、144Hz 下 1 秒后的位移都是 30 DIP，不存在频率对齐问题。亚像素误差会被 egui 渲染管线自动四舍五入，视觉上不可察觉。

#### 滚动开始 / 停止 / 文本变更

| 事件 | 行为 |
|------|------|
| `current_line.text` 变化后，新文本宽度 > 窗口宽度 | **立即**开始滚动，`offset_x` 从 `window_width` 起算（文本左边缘在窗口右边缘外，刚好在屏幕外侧，准备向左进入）。**UDP 触发的歌词切换**重置 `offset_x` |
| 新文本宽度 ≤ 窗口宽度 | 居中显示，`offset_x = 0`，不滚动 |
| 滚动中文本变更（切歌 / 换行） | `offset_x` **立即重置**为 `window_width`，下一帧从右边缘外开始滚动新文本 |
| 滚动中窗口宽度变化（用户改 width 配置） | 重新计算文本宽度；满足「文本宽 > 窗口宽」则继续滚动，`offset_x` 不重置（避免跳动）。 |
| 滚动中字体 / 字号变化（reload 触发） | **重置 `offset_x = window_width`**（视为切歌等价）。原因：字体变化导致 text_width 重测，旧 offset_x 与新 text_width 不再对应，终点判定（`-text_width`）会失效 |

**`offset_x` 语义**：文本左边缘 X 坐标与窗口左边缘 X 坐标之差（单位 DIP）。正数 = 文本在窗口右侧；负数 = 文本在窗口左侧。起点 `window_width`（文本刚好从屏幕外右侧出现）；终点 `-text_width`（文本左边缘离开屏幕左侧）。到达终点后下一轮重置为 `window_width`，循环往复。

> **永不销毁内存歌词**：进程启动初始化时 `current_line.text = "......"`。UDP 数据到达就更新，没数据就保持上一次的值。用户切歌、播放器断开均不影响。

### 7.4 拖动

| 触发条件 | 行为 |
|---------|------|
| `locked = false`，鼠标左键**在窗口任意像素**按下并移动 | 窗口跟随光标移动（无最小位移阈值，按下即开始） |
| `locked = false`，鼠标左键按下但未移动 | 不视为拖动，松开后无任何状态变化 |
| `locked = true`，鼠标左键按下 | 完全不响应，窗口不动 |
| 鼠标右键点击歌词窗口 | 不响应（仅托盘图标响应右键） |

> 由于窗口内容仅有歌词文本且全部背景透明，"按住哪里"没有差别——歌词文字上、空白处、窗口边缘像素都是一样的可拖动区域。

**拖动结束后**：
- 新的 `(pos_x, pos_y)` **仅保存在主进程内存**，**不写入任何文件、不通知设置进程**。
- 落盘时机见下方 "拖动结果的落盘路径"。

#### 拖动结果的落盘路径

主进程**不直接写 `config.toml`**（避免与设置进程抢写），拖动结果通过以下方式之一持久化：

| 路径 | 触发 | 行为 |
|------|------|------|
| **用户主动打开设置页 → 保存** | 用户右键托盘 → 配置 → 修改任意字段 / **或不修改只点保存** → 点保存 | 设置进程启动时通过命名管道 `\\.\pipe\\lyric-for-musicfox-pos` 询问主进程当前 `(pos_x, pos_y)`，合并到表单 → 写入 `.tmp` → 校验 → 原子 rename 为 `config.toml`。**即使用户不修改任何字段、只点保存，pos 也会被刷新到磁盘**（这是预期，避免位置与拖动结果长期不同步） |
| **用户只拖动不打开设置页** | 拖动完成后用户继续听歌、不打开设置 | 主进程内存位置不落盘；下次主进程重启时位置丢失 |

> **设计权衡**：拖动结果不自动持久化（避免主/设进程抢写），代价是用户若拖动后不打开设置页直接关闭主进程，位置会丢。这是为简化同步机制付出的代价。

- `locked = true` 时仍可通过**修改配置文件**来改变位置（详见 §10 设置界面），只是不能通过鼠标拖动。

### 7.5 占位符

- 进程启动到首次收到 UDP 数据之间的占位：透明度 0.3 的 `"......"`。
- 收到数据后占位消失，按 §7.3 规则渲染。
- 文本本身是 `"......"`（6 个半角点号）。

---

## 8. 窗口与歌词样式配置

| 字段 | 类型 | 默认值 | 校验 |
|------|------|--------|------|
| `width` | `u32` | `800` | 100 ≤ x ≤ 4000 |
| `height` | `u32` | `80` | 20 ≤ x ≤ 500 |
| `pos_x` | `Option<i32>` | `None` | `Some` 时 -2^31 ≤ x ≤ 2^31-1（i32 全范围；3 屏 4K 横排坐标仍可表达） |
| `pos_y` | `Option<i32>` | `None` | `Some` 时 -2^31 ≤ x ≤ 2^31-1（i32 全范围） |
| `stay_on_top` | `bool` | `true` | `false` 时歌词窗口不置顶（可能被其他窗口遮挡） |
| `frame_less` | `bool` | `true` | `false` 时歌词窗口带默认窗口装饰（标题栏、关闭按钮），但歌词文本仍透明。**拖动语义**：只有 `locked=false` 时才允许拖动；`frame_less=false` 时拖动限制为**标题栏区域**（歌词区不接受鼠标事件以免与标题栏拖动冲突）。**不推荐关闭** |
| `locked` | `bool` | `false` | — |

> **窗口大小仅可通过设置页修改**：歌词窗口本身不提供拖拽边缘调整大小能力（§7.1）。设置页提供 `width` / `height` 数值输入框（带 DIP 单位提示），保存后通过 IPC 通知主进程重建。`pos_x` / `pos_y` 同理仅在设置页改"值"；"实际位置"由鼠标拖动决定（详见 §7.4 + §10.6）。

| 字段 | 类型 | 默认值 | 校验 |
|------|------|--------|------|
| `font_family` | `String` | `"Microsoft YaHei"` | 必须是系统已安装字体名 |
| `font_size` | `f32` | `24.0` | 6.0 ≤ x ≤ 200.0 |
| `font_bold` | `bool` | `false` | — |
| `font_italic` | `bool` | `false` | — |
| `font_color` | `String` | `"#ffffff"` | 严格 `#RRGGBB` |
| `font_outline_color` | Option 字符串 | `Some("#000000")` | `Some` 时严格 RRGGBB 七字符（七字符：井号 + 六位十六进制）；`None` 表示不描边，与 `width = 0` 等价（运行时取 OR：任一为「无描边」则不绘制）。**TOML 表示**：`None` 用 **`font_outline_color = ""`**（空字符串）表示（TOML 无原生 null；与 §12.1 字段缺失策略区分，Option 字段**不走**缺字段默认）；**字段缺失走 `Default::default()` → `Some("#000000")`**（默认描边色） |
| `font_outline_width` | `u32` | `1` | 0 ≤ x ≤ 16，单位 **DIP**（逻辑单位，DPI 缩放下等比放大） |

### 8.1 字体枚举

- 设置进程启动时枚举系统所有已安装字体，填充下拉列表。
- 列表显示字体族名（family name），存储为字符串。
- 若当前 `config.toml` 中的 `font_family` 在新机器上不存在 → 回退默认 `"Microsoft YaHei"`，并在设置页用红色提示。

### 8.2 颜色格式

- 仅接受 `#RRGGBB`（7 字符），不区分大小写。
- 解析后内部存储为 `RGBA`（alpha 固定 255，描边 alpha = 255）。
- **落盘策略**：始终小写化为 `#rrggbb` 写入配置文件（`#FFFFFF` → `#ffffff`）。

---

## 9. 系统托盘

### 9.1 行为

| 触发 | 行为 |
|------|------|
| 左键点击 | 启动/激活设置进程（详见 §4.2） |
| 右键点击 | 弹出菜单（含"配置"项，同样调用启动/激活设置进程） |

**托盘图标资源**：嵌入到 .exe 的 PE 资源段（使用 `winres` crate 在 build.rs 里嵌入 .ico 文件），运行时从资源加载为 `tray-icon::Icon`。资源 ID：`IDI_ICON1`（正常）+ `IDI_ICON1_GRAY`（解析失败闪烁时使用）。本项目不依赖任何运行时外部图标文件。

> 左键与右键菜单的"配置"**语义等价**，均触发"启动或激活设置进程"流程，仅交互方式不同（左键单击、右键点击菜单项）。

### 9.2 右键菜单

```
┌──────────────┐
│  配置         │  ← 启动/激活设置进程
│  ─────────  │
│  退出         │  ← 关闭主进程（同时销毁歌词窗口、清理托盘）
└──────────────┘
```

### 9.3 退出

- 关闭主进程：销毁歌词窗口、移除托盘图标、关闭 UDP socket、释放 Mutex。
- 设置进程独立退出不影响主进程。

---

## 10. 设置界面（独立进程）

### 10.1 启动

- 命令：`lyric-for-musicfox.exe --settings`
- 单实例：若已有设置进程运行 → 通过窗口标题 `lyric-for-musicfox - 设置` 调用 `FindWindowW` 查找句柄 → `SetForegroundWindow` 激活到前台，当前进程立即退出。
- 设置进程窗口标题固定为 `lyric-for-musicfox - 设置`（中文 UI），便于主进程/其他实例定位。
- **主进程歌词窗口标题**：`lyric-for-musicfox`（无后缀）。不依赖该标题做 FindWindow（主进程不需要被定位）；仅用于任务栏 / Alt+Tab 显示。
- 启动时尝试读取 `%APPDATA%/lyric-for-musicfox/config.toml`（**仅设置进程负责写盘**；主进程不写 config.toml）：
  - **不存在** → **非阻塞 toast**（设置窗口右下角角标，3 秒后自动消失）提示「正在创建配置文件」；同时将默认配置写入磁盘。设置窗口可正常编辑，不需要关闭 toast。
  - **存在但解析失败** → 弹出错误对话框，列出错误位置；提供"使用默认配置重置"和"取消"两个按钮。

- **主进程启动时** config.toml 缺失：静默用 `Config::default()` 运行，不写配置文件，不 toast（主进程不写配置）。验证项见 dev.md §2 P0 验收。
    - **取消**：设置进程退出，不创建任何文件。
    - **使用默认配置重置**：备份原 `config.toml` 为 `config.toml.bak`（同目录），写入默认配置到 `config.toml`，表单加载默认值，UI 可继续编辑。`.bak` 保留至用户手动删除。
- 启动后**同步拉取主进程窗口当前位置**：
  - **拉取时机**：设置窗口显示之前同步拉取，UI 渲染阻塞等待（最多 1s）；超时后窗口打开，显示 `config.toml` 旧值。
  - 通过命名管道 `\\.\pipe\lyric-for-musicfox-pos-{session_id}` 询问"当前 pos_x/pos_y"
  - 若主进程在运行且响应 → 表单中 `pos_x` / `pos_y` 显示主进程内存值（最新拖动结果）
  - 若主进程未运行 / 管道无响应 → 表单显示 `config.toml` 中的旧值，**表单位置字段右侧显示提示**「上次磁盘保存值，主进程未运行」灰色文字
  - **`None` 在表单的表示**：`pos_x` / `pos_y` / `font_outline_color` 输入框为空字符串时表单内为 None（拉取到 `EMPTY,EMPTY` 或旧 `""`）；输入框为空 + 保存 → 落盘为 `pos_x = ""`（TOML 显式空串，运行时解析为 None）
  - 用户保存时把表单中的位置一并写入 `config.toml`（即使用户没主动改位置字段，保存操作也会「刷新」位置为拉取到的主进程内存值；这是预期行为，避免位置与拖动结果长期不同步）

### 10.2 编辑流程

```
┌──────────────────────────────────────────────────────────────┐
│  设置进程启动                                                 │
│      │                                                        │
│      ├─ 拷贝 config.toml → config.toml.tmp (同目录)          │
│      │  若 .tmp 已存在 → 优先用 .tmp (覆盖 config.toml)        │
│      │  若 config.toml 不存在 → 先创建 config.toml（写入默认配置）再拷到 .tmp │
│      │  若 .tmp 与 config.toml 同时存在 → 以 .tmp 为准（保留未保存编辑）   │
│      │  动机：保证 .tmp 与 config.toml 始终同源，便于崩溃恢复逻辑判断
│      ▼                                                        │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  设置 GUI 读取 .tmp 到内存表单                         │    │
│  │  表单变更 → 防抖 500ms → 序列化写回 .tmp               │    │
│  └──────────────────────────────────────────────────────┘    │
│      │                                                        │
│      │  点击"保存"                                            │
│      ▼                                                        │
│  1. 校验 .tmp 内存表示的所有字段 (见 §10.4)                  │
│  2. 校验失败 → 红字提示, .tmp 不动, 不写回                   │
│  3. 校验通过 → 原子写回 config.toml (rename .tmp → 正式文件)   │
│  4. 通过 IPC 通知主进程重载                                    │
│  5. 保存后 .tmp 保持与 config.toml 一致                      │
└──────────────────────────────────────────────────────────────┘
```

> **设计动机**：`.tmp` 作为隔离层，避免设置进程异常退出时污染正式配置；防抖避免高频输入造成 IO 抖动；原子 rename 保证校验与写入的原子性。

#### 崩溃恢复

| 场景 | 行为 |
|------|------|
| 设置进程在防抖期间崩溃 | `.tmp` 可能未刷新（或停留在上次防抖成功的状态）；下次启动设置进程时 → 直接读取 `.tmp`，**不覆盖**（保留未保存编辑） |
| `.tmp` 存在但内容损坏（解析失败） | 丢弃 `.tmp`，从 `config.toml` 重新拷贝；在日志记录 warn |
| `.tmp` 与 `config.toml` 同时存在但不一致 | 以 `.tmp` 为准（视为"用户上次未保存的工作副本"），`.tmp` 不重置；仅当 `.tmp` 解析失败时才回退到 `config.toml` |
| `config.toml` 损坏 + `.tmp` 也损坏 + `.bak` 存在 | 三者同时存在的场景：`.bak` 是上次重置时的备份，**优先级最低**（兜底），`.tmp` > `config.toml` > `.bak`。三个都损坏 → 回退默认配置 + 重新创建 .bak |

### 10.3 写入策略：防抖 500ms + 保存原子写

- **防抖目的**：避免高频输入（如拖动滑块、连续键击）造成 IO 抖动；提供 500ms「反悔窗口」让用户连续编辑。
- 表单字段变更时**不立即写盘**，防抖 **500ms** 后才序列化到 `config.toml.tmp`。
- 防抖期间字段值在内存中累积；任何新变更重置计时器（最后一次变更后 500ms 才落 `.tmp`）。
- **不区分字段类型**：文本 / 数值 / 颜色 / 复选框统一走防抖。
- **保存按钮与防抖的交互**：点击"保存"时 → ① **取消防抖计时器** → ② **立即同步 flush 内存表单到 `.tmp`**（不依赖防抖到期）→ ③ **从 `.tmp` 读回** → ④ 校验所有字段 → ⑤ 原子 rename 为 `config.toml`。保证 `.tmp` 与内存、磁盘三者一致。
- **保存失败保留内存修改**：若步骤 ②-④ 任一步失败（如磁盘满、权限丢失）→ 内存表单保留用户输入，`.tmp` 状态可能不一致（但下次防抖会被覆盖）。**不自动回滚内存**，避免丢用户输入；用户看到错误提示后可重试或关闭窗口（弹「是否丢弃」）。
- 关闭设置进程窗口时：若有未提交到 `.tmp` 的变更 → 弹**模态确认对话框**「是否丢弃未保存的修改？」提供「保存」/「丢弃」/「取消」三个选项：保存走保存流程，丢弃则删除 `.tmp`，取消则取消关闭动作。
- **未保存变更的判定标准**：与 **内存表单**（egui UI 状态）相比，不是与 `.tmp` / `config.toml` 比。每次表单字段被修改时设置进程记一个内存 dirty flag；dirty flag 为 true 时关闭 → 弹上面模态确认。`.tmp` 是防抖写入的磁盘镜像，与 dirty 独立。

### 10.4 校验规则

| 字段 | 规则 |
|------|------|
| `width` | 100 ≤ x ≤ 4000 |
| `height` | 20 ≤ x ≤ 500 |
| `pos_x` / `pos_y` | `None` 或 i32 全范围（-2^31..2^31-1） |
| `font_family` | 系统已安装字体列表中存在 |
| `font_size` | 6.0 ≤ x ≤ 200.0 |
| `font_color` | 匹配 `^#[0-9a-fA-F]{6}$` |
| `font_outline_color` | `None` 或匹配 `^#[0-9a-fA-F]{6}$` |
| `font_outline_width` | 0 ≤ x ≤ 16 |

校验失败时在对应输入框下方显示红字提示，不阻塞其他字段编辑。

### 10.5 "打开配置文件夹"按钮

- 设置窗口**始终**显示一个"打开配置文件夹"按钮（不是仅首次）。
- 点击 → 调用 `explorer.exe` 打开 `%APPDATA%/lyric-for-musicfox/` 目录。
- 用户用外部编辑器修改 `config.toml` 后：
  - 设置进程**不主动检测**外部变更（避免冲突）。
  - 下次启动设置进程或重启主进程时会读到新值。
  - 主进程运行期间外部修改**不生效**。
- **主进程配置加载策略**：主进程启动时**一次性读 `config.toml`**；运行期间不轮询、不重读；只依赖 §10.6 的命名管道信号触发样式重建。**位置字段由主进程独占**（§7.4）：reload 重载**不重读** pos_x/pos_y 文件值（即使 config.toml 中位置变更，运行中主进程内存的 pos 仍是拖动结果）。用户想主动套用 config.toml 中的位置 → 需重启主进程。

### 10.6 主进程收到的"重载"信号

| 通知方式 | 说明 |
|---------|------|
| **Windows 命名管道** | 命名 `\\.\pipe\lyric-for-musicfox-reload`；设置进程写入**任意 1 字节**后断开（字节内容不携带配置版本号 / 时间戳，语义仅为"有重载事件"）；主进程在独立线程 `ConnectNamedPipe` 循环监听 |
| **信号丢失检测** | 不做主动检测。Windows 命名管道保证：connect 成功 + 写入成功 + 断开 → 主进程几乎必然收到。反例场景（缓冲区满 / 主进程刚好在处理别的 IO）极少，加重试逻辑复杂度收益不匹配。丢失兜底：重启主进程 |
| **重载进行中收到新信号** | 丢弃。设置进程连续多次保存会被合并为一次最终样式；中间状态不需保留 |
| **重载进行中的判定** | GUI 主线程维护 is_reloading 与 is_dragging 两个 AtomicBool。reload 监听线程收到信号时检查 is_dragging：true 则丢弃；否则读 config.toml + 写 needs_reload AtomicBool → 下一帧 vsync tick 重建样式 → 重置 needs_reload。重建期间收到新信号 → 丢弃 |
| 主进程收到信号 | 重载流程见下表 |

#### 重载流程

| 步骤 | 操作 |
|------|------|
| 1 | 重新读取 `config.toml` 到内存中的 `LoadedConfig` |
| 2 | 重建字体与渲染上下文（字体族 / 字号 / 颜色 / 描边 / 加粗 / 斜体） → 下一帧 vsync tick 立即以新样式渲染，不需要等 UDP 包 |
| 3 | 重设窗口尺寸（`width` / `height`），如窗口已超出可见区则裁剪到主显示器内 |
| 4 | 窗口位置 **不变**（取主进程内存值，避免拖动结果被覆盖） |
| 5 | 拖动期间收到信号 → 丢弃该信号，跳过本次重载；拖动结束后下一次信号才生效 |
| 6 | **重载期间 config.toml 不可读**（被占用、权限丢失、损坏）：保留当前内存中的 `LoadedConfig`，记 error 日志，**不重建样式**。下一次重载信号会重试 |

> 拖动期间判定：GUI 主线程维护 `is_dragging: AtomicBool`，左键按下 + 移动期间为 true，松开时立即置 false；重载监听线程在收到信号时检查该标志决定是否丢弃。
>
> **字体变更后滚动状态**：重载步骤 2 重建字体后视为「切歌等价」 → `offset_x` **重置为 `window_width`**（与 §7.3 表格一致）。原因：字体变化导致 text_width 重测，旧 offset_x 与新 text_width 不再对应，终点判定失效。

> **设计动机**：位置字段由主进程独占（拖动产出，§7.4），样式字段由设置进程独占（配置产出），两者生命周期完全解耦，避免任一方在对方编辑中互相覆盖。

---

## 11. 系统配置

| 字段 | 类型 | 默认值 | 校验 |
|------|------|--------|------|
| `receive_port` | `u16` | `16501` | 1024 ≤ x ≤ 65535 |
| `send_port` | `u16` | `16502` | 1024 ≤ x ≤ 65535 |

未来如需变更端口，可通过修改此字段实现。当前版本 UI 不暴露此配置。

**生效时机**：**仅主进程启动时生效**。`receive_port` / `send_port` 修改 → 重启主进程；运行期间通过 reload 信号不重载该字段（避免主进程已 bind 的 UDP socket 失效）。`§6.1` 表格中"接收端口 16501 / 发送端口 16502"是默认值，`§11` 字段值优先；二者不一致时以 `§11` 字段为最终值。

---

## 12. 配置文件格式

### 12.1 路径

- 路径**硬编码**为 `%APPDATA%/lyric-for-musicfox/config.toml`，不提供环境变量覆盖。
- 编码：UTF-8 无 BOM。
- 例如：用户名占位符 + Roaming + lyric-for-musicfox/config.toml（具体路径形如 C:/Users/你的用户名/AppData/Roaming/lyric-for-musicfox/config.toml）
- 决策依据：本项目只面向 Windows，不需要 Linux/macOS 风格的 XDG_CONFIG_HOME 抽象；多一层配置会让用户更难排查问题。

#### 字段缺失策略

- 字段缺失（如 window 下没有 width）：走 Default::default()，不报错；启动后以默认值运行。
- 字段类型错误（如 width 是字符串）：TOML 解析失败，按 §14 流程处理：主进程 stderr + exit code 1（P0 起；P4 才增加托盘闪烁），设置进程 P1+ 弹错误对话框。
- Option 字段类型错误（如 font_outline_color 是数字）：同上解析失败，不走 Option 默认的 None，因为字符串类型不匹配是语义错误。
- 字段值越界：按 §10.4 校验规则；启动时校验失败同样回退默认。
- Option 字段为 None：TOML 用**显式空值**表示（如 `pos_x = ""` 或 `font_outline_color = ""`）。**Option 字段不走字段缺失策略**（缺字段 → 走 Default → Some(默认值)）。与 §8 `font_outline_color` 约定一致。

### 12.2 示例

```toml
# lyric-for-musicfox 配置文件
# 由设置界面生成；手工修改后需重启主进程生效
# 注：本示例展示的是「默认生成值」，与 §7.2 / §8 表中的默认定义一致。

[window]
width = 800
height = 80
# pos_x / pos_y 为空表示「居中 / 顶部偏下 100 DIP 转物理像素」，TOML 无原生 null，**字段缺失或 `pos_x = ""` 都表示 None**（走 pos_x 默认 None）
stay_on_top = true
frame_less = true
locked = false

[lyric_style]
font_family = "Microsoft YaHei"
font_size = 24.0
font_bold = false
font_italic = false
font_color = "#ffffff"
font_outline_color = "#000000"
font_outline_width = 1

[system]
receive_port = 16501
send_port = 16502

# Option 字段 None 用「显式空字符串」表示，与缺字段（→走默认）区分：
# pos_x = ""        ← 表示 None（启动时主显示器居中）
# font_outline_color = ""  ← 表示 None（不描边，与 width=0 等价）
# 不要写 `pos_x = null`（TOML 无原生 null）
# 字段缺失（不写 pos_x 这一行）走 Default::default() → 同样得到 None（pos_x 默认就是 None）
# font_outline_color 字段缺失走 Default → Some("#000000")（默认描边色，不是 None）
```

---

## 13. 非功能需求

| 类别 | 要求 |
|------|------|
| **性能 - 内存** | 主进程空闲 ≤ 50 MB；设置进程空闲 ≤ 80 MB |
| **性能 - CPU** | 主进程空闲 ≤ 1%（100% 缩放，1080p）；UDP 数据到达时主线程开销忽略不计 |

**「空闲」定义**：
- **内存空闲**：进程启动 5 秒后、无 UDP 数据到达、无用户交互的稳态内存占用（含运行时、egui 字体缓存、托盘图标资源）
- **CPU 空闲**：上述内存空闲条件下 5 秒窗口内的平均 CPU 占用
- **UDP 持续场景**：go-musicfox 持续推送（假设 100ms/包、滚动状态）的稳态 CPU 仍需 ≤ 3%（验收目标）。每包解析 < 1ms + vsync 渲染开销 ≈ 1-2% CPU
- **滚动持续场景**：歌词超过窗口宽度时持续滚动 + UDP 推送同时存在时 CPU ≤ 3%；仅滚动（无 UDP）时 CPU ≤ 2%。每帧 layout 重计算 < 500 μs
| **性能 - 渲染** | 歌词窗口刷新率跟随 egui 默认（vsync），不做主动节流 |
| **响应** | UDP 数据到达 → 屏幕渲染 **理论目标 ≤ 50 ms**（数据到达 → 内存状态更新 → 下一帧 vsync 渲染可见）；**验收阈值 1 s**（包含 IO + IPC + 渲染管线全链路延迟裕量） |
| **可执行文件** | 单个 `.exe`，静态链接，无外部 DLL 依赖；最大体积 **≤ 40 MB**（egui + tray-icon + windows 静态链接后实际约 25-35 MB；预留裕量；**不使用 tokio**，所有 IO 用 `std::thread::spawn` + 阻塞 IO）。release profile 启用 `lto = true`、`codegen-units = 1`、`panic = "unwind"`、`strip = true`、`opt-level = 3`（与 §13 「崩溃」行一致，保证 Drop 运行；启动时间优先） |
| **启动时间** | 主进程冷启动 ≤ 1 s（指进程开始执行到歌词窗口**首次绘制**的 elapsed，**不含** OS 加载 PE 资源时间） |
| **日志** | 单文件 `%APPDATA%/lyric-for-musicfox/log.txt`，超过 5 MB 时**重命名为 `log.txt.old` 并新建 `log.txt`**（仅保留最近一个历史，约 10 MB 上限）；重命名前调 `flush` 确保所有 buffer 落盘；若 `log.txt.old` 已存在则覆盖。使用 `tracing-appender` 的 non-blocking writer。**写不进去时降级**：若日志路径不可写（权限不足 / 磁盘满）→ 降级到 `stderr` 输出，不阻塞进程启动。**默认日志级别**：`INFO`（生产）/ `DEBUG`（dev profile，可由环境变量 `LYRIC_LOG=debug` 启用）。`--benchmark` 启用 `TRACE`（延迟统计需要）。详细事件（UDP 包字段、字体度量）走 `TRACE` 仅在 dev/benchmark 出现 |
| **国际化** | **UI**：仅简体中文；**日志**：英文（便于检索与跨语言调试） |
| **崩溃** | 主进程退出路径必须清理：① 托盘图标 `remove()` ② Mutex `ReleaseMutex` ③ UDP socket 关闭 ④ 命名管道断开。使用 `Drop` 实现 RAII；事件循环 panic 时由 `winit` 的 `EventLoop::run` 返回值传递；unsafe 边界（如 `windows` crate FFI）出错返回 `Result` 向上传播。**release profile 使用 `panic = "unwind"`**（而非 abort），保证 Drop 能运行；体积损失约 +1-2 MB 可接受 |
| **可观测性** | 关键事件（启动/退出/UDP 接收/配置重载/校验失败）记日志 |
| **日志格式** | 纯文本（非 JSON）：`{ISO8601 timestamp} {LEVEL} [{module}] {message}`，例如 `2026-01-15T10:23:45.123Z INFO  [lyric::udp] received lyric_update: foo - bar`。使用 `tracing-subscriber` 的 `fmt::layer` |

#### 线程模型（主进程）

- **GUI 主线程**：eframe/winit 事件循环、歌词渲染、拖动处理
- **UDP 接收线程**：绑定 0.0.0.0:16501，阻塞 recv；解析后**直接写锁 `Arc<RwLock<LyricState>>`**（**不用 channel**，详见 §6.2）
- **命名管道监听线程**：reload / pos 各一个（主进程）；presence-{session_id} 一个（设置进程）
- **Presence 监听线程**（设置进程）：循环 `ConnectNamedPipe` 接收新实例激活请求
- **显示器监控线程**：每 5s EnumDisplayMonitors 检测主显示器变化
- **日志 worker 线程**：tracing-appender non-blocking writer 的后台 flush 线程
- **tray-icon 回调线程**：操作系统触发 → **通过 crossbeam_channel 推送到 GUI 主线程消费**（唯一使用 channel 的场景，因为 tray-icon 库不允许在回调线程 spawn 设置进程或调 ipc）

线程间通信统一原则：跨线程状态传递用 `Arc<RwLock<T>>`（state）/`Arc<AtomicBool>`（信号量）/ `crossbeam_channel`（仅 tray 回调）；GUI 主线程不在事件回调里做重活。

---

## 14. 错误处理与边界

| 场景 | 行为 |
|------|------|
| 端口 16501 被占用 | 发生在 Mutex 检查通过之后：bind 失败 → 判定为「别的不明程序占用 16501」→ **报错退出**（exit code 1，日志记录错误）。**不视为本项目已有实例**，因为本项目的 Mutex 检查已通过 |
| `%APPDATA%` 不存在或无写权限 | 启动失败；写 stderr（包含错误详情）+ exit code 2；考虑到主进程可能是开机自启 / 无窗口模式，**不弹窗** |
| `config.toml` 解析失败 | **P0 主进程**：stderr 记录详情 + exit code 1（无 GUI/托盘）；**P1-P3 主进程**：使用默认配置继续启动，记 error 日志（无托盘）；**P4+ 主进程**：使用默认配置继续启动 + 托盘图标闪烁提示（规格见下表）；设置进程 P1+：见 §10.1 |

#### 托盘闪烁规格（解析失败时）

- 频率：1 Hz（正常与半透明灰各 500ms 切换）
- 「半透明灰」具体值：托盘图标 RGBA 设为 `(128, 128, 128, 128)`，实现上可通过 `tray-icon::Icon` 的两套图标资源（正常 + 灰版）切换
- 持续：不自动停止，**仅重启主进程时才重置**（即使中途外部修复 config.toml，主进程也不重新检测）
- 同时日志记 error 级别，包含文件路径与解析错误详情
| UDP 数据格式错误 | 丢弃该包，记 warn 日志，不影响后续 |
| UDP 包 > 8 KB | recv buffer 限制 8 KB；超过丢弃，记 warn 日志 |
| UDP 包 `type` 字段 ≠ `lyric_update` | 丢弃，记 warn 日志 |
| UDP 包字段缺失（如缺 `current_line`） | 走 §12.1 字段缺失策略：缺字段用 `Default::default()`；后续渲染依据默认值 |
| UDP 丢包 | 不回放、不插值；下一包到达后直接跳到该包携带的 `current_line.text` |
| go-musicfox 短暂不发包 | 不阻塞、不卡住：渲染永远显示内存中最新的 `current_line.text`（§7.3「永不销毁内存歌词」）；切歌由 go-musicfox 推送新包触发，本进程不主动检测 |
| `font_family` 在新机器不存在 | 回退 `"Microsoft YaHei"`，设置页红字提示；**主进程启动时主动调用 fontdb 检查**（不等到 UDP 第一帧渲染时才报错）|
| 屏幕分辨率变化 | **首选事件驱动**：在窗口过程 hook `WM_DISPLAYCHANGE` 消息；**轮询兜底**：后台任务每 5s 调 `EnumDisplayMonitors` 检测主显示器工作区变化。任一触发 → 重新裁剪歌词窗口坐标到主显示器可见区 |

**轮询兜底场景**：事件驱动 hook 在以下场景下不可靠，需轮询兜底：
- **虚拟桌面切换**（Win+Ctrl+D 创建/切换桌面）：`WM_DISPLAYCHANGE` 不一定触发
- **远程桌面会话重连**：RDP 重连后窗口可能跨会话边界，hook 失效
- **DPI 动态切换**（设置 → 显示 → 缩放）：部分驱动不触发 `WM_DISPLAYCHANGE`，需轮询 `GetDpiForMonitor` 检测
- **第三方工具修改显示配置**（如 f.lux、多显示器管理工具）：可能不通过标准消息路径

实现上二者**同时启用**：事件驱动作为低延迟主路径，轮询作为 5s 周期的兜底；任一检测到变化即触发裁剪。
| DPI 缩放变化 | **仅重建字体资源**（egui FontDefinitions + egui Context 重设），**保留 winit 窗口句柄与位置**；期间窗口持续可见，仅字体度量重测期间可能出现 ≤ 50ms 短暂白屏。**不销毁 eframe 事件循环**；实现上调用 eframe 的 `set_fonts` API（具体实现需验证 eframe 内部对 egui Context 的所有权管理）。 |
| 用户拖动到副屏 | 记录**虚拟桌面绝对坐标**（跨屏连续），重启后精确还原到该位置；主显示器变更 / 副屏拔除时启动裁剪到新主显示器可见区。**裁剪规则**：`pos_x` 超主屏右边界 → 设为 `monitor_width - window_width`；`pos_x` 超左边界 → 设为 `0`；垂直同理。裁剪**仅修改主进程内存中的位置**，**不写入 `config.toml`**（遵守 §4.2 「主进程不直接写 config.toml」原则），待用户下次打开设置页保存时一并持久化。`pos_x = None` 不参与裁剪（启动时会立即计算为具体坐标，所以「运行时 pos_x = None」几乎不可达）；若启动后状态变为 None（如配置被外部清空），下一次 vsync tick 重新计算居中坐标。
| 拖动后未打开设置页就关闭主进程 | 位置丢失，下次启动回到 `config.toml` 中的旧值。这是 §7.4 "拖动结果不自动持久化" 的代价 |
| 设置进程启动时主进程无响应 | 表单显示 `config.toml` 中的旧位置；用户保存后用旧值，不阻塞 |
| 设置进程保存时主进程异常 | 设置进程仍完成写盘；下次启动主进程生效 |
| 拖动中收到"重载"信号 | 丢弃该信号，**不重建窗口**；主进程内存中的拖动结果保留，不受重载影响（拖动结束才走"待保存状态 → 设置页保存"流程） |

---

## 15. 验收清单

- [ ] 双击 `lyric-for-musicfox.exe` 启动后 1 s 内出现透明窗口
- [ ] 启动时窗口显示 `"......"` 占位符
- [ ] go-musicfox 播放时歌词实时更新并自动滚动
- [ ] `locked = false` 时，鼠标在窗口任意像素（歌词文字上 / 空白处）按住左键可拖动窗口
- [ ] `locked = true` 时，鼠标在歌词窗口上点击/拖动**完全无响应**，事件穿透到下层应用
- [ ] 拖动窗口到任意位置后，**打开设置页 → 点保存** → 关闭主进程 → 重启后位置保留（前置条件：必须经过一次设置页保存，未保存的拖动结果丢失，见 §7.4）
- [ ] 右键托盘 → 配置 → 设置窗口出现；再次右键 → 配置 → 设置窗口**被聚焦**而非新建
- [ ] 设置中改字号 → 保存 → **理论目标 ≤ 50 ms（vsync 内可见），验收阈值 1 s**（留 IO + IPC + 渲染管线全链路延迟裕量）
- [ ] 设置中改字体 → 保存 → 歌词窗口字体变化（前提是字体已安装） |
- [ ] 设置页保存后字体被卸载 → 主进程重建字体失败 → 自动降级到默认字体（Microsoft YaHei），记 warn 日志，不崩溃 |
- [ ] 同时启动两个 `lyric-for-musicfox.exe --settings`，第二个激活第一个并立即退出
- [ ] 启动两个 `lyric-for-musicfox.exe`，第二个因 **Mutex 已存在** 静默退出（exit code 6，stderr 一行 "another instance running"）；如果别的不明进程占用 16501（且本项目 Mutex 不存在），则提示「16501 端口被不明程序占用」到 stderr 并退出码 1（双击场景托盘闪灰图标） |
- [ ] 删除 `config.toml` 后启动设置进程，弹出"正在创建配置文件"提示
- [ ] 删除 `config.toml` 后启动主进程：主进程**静默用 `Config::default()` 运行**（不创建 config.toml；主进程不写配置文件、仅在启动时读一次，详见 §10.1）
- [ ] 设置页"打开配置文件夹"按钮可打开 `%APPDATA%/lyric-for-musicfox/`
- [ ] 修改颜色为非法值（如 `#fff`）点击保存，在颜色输入框下方显示红字提示，配置不被写入
- [ ] 主进程体积 ≤ 40 MB；空闲内存 ≤ 50 MB；空闲 CPU ≤ 1%；**测量方法**：Windows 任务管理器观察 Working Set（内存）+ 进程 CPU 曲线稳态值；脚本可写 `Get-Process lyric-for-musicfox | Select WorkingSet,CPU` 验证
- [ ] UDP 数据到达 → 渲染 **≤ 50 ms 理论目标 / 1 s 验收阈值**；**测量方法**：开发期在 UDP 处理函数入口 / 渲染函数入口打 `tracing::trace!` 时间戳，对比日志。预留 `--benchmark` 参数输出延迟统计
- [ ] **设置页保存后字体被卸载** 场景：手动验收项（运行时卸载系统字体需要手工干预），不纳入 CI 自动化测试

---

## 16. 版本

- 当前：**0.2.0**（重构版）

#### 版本变更日志

| 版本 | 变更 |
|------|------|
| 0.1.0 | 初版需求描述 |
| 0.2.0 | 重构需求文档：明确双进程模型 + 配置 IPC 机制 + 副屏支持 + 出站协议预留 + 多项边界处理 |

- 许可证：MIT

#### 依赖版本约束策略

- Cargo.toml 中所有依赖使用 **caret 约束**（如 `egui = "0.28"`），允许 semver 兼容升级；lock 文件提交到仓库保证可复现构建。
- 主依赖版本不锁定到具体 patch 版本，避免安全补丁难以合并。

#### build.rs 图标资源

- 图标源文件位于仓库 `assets/icon.ico`（正常）与 `assets/icon_gray.ico`（解析失败闪烁用）。
- `build.rs` 使用 `winres = "0.1"` crate 的 `WindowsResource::new().set_icon("assets/icon.ico")` 嵌入正常图标；使用 `set_icon_with_id("assets/icon_gray.ico", "IDI_ICON1_GRAY")` 嵌入灰图标。托盘运行时从资源 ID `IDI_ICON1` / `IDI_ICON1_GRAY` 加载。
- CI 构建时要求 Windows runner（winres 仅在 Windows 编译期生效）；Linux/macOS runner 跳过该步骤，cross-compile 不可用。

---

## 附录 A：依赖概览（参考）

| 用途 | Crate |
|------|-------|
| GUI | `egui`, `eframe` |
| 系统托盘 | `tray-icon` |
| 字体枚举 | `fontdb` |
| 配置 | `serde`, `toml` |
| 异步 | *(无 — 所有 IO 用 `std::thread` + 阻塞 IO)* |
| 日志 | `tracing`, `tracing-appender` |
| Windows API | `windows` crate |
| 单实例 / 命名管道 | `windows` crate（`CreateNamedPipeW`, `CreateMutexW`） |

## 附录 B：交付物

- `lyric-for-musicfox-x86_64-pc-windows-gnu.exe`（单一可执行文件）
- `LICENSE`（MIT）
- `README.md`（使用说明 + 截图）
