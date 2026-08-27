# lyric-for-musicfox 阶段三开发设计与实施手册 (dev.md)

> 版本：0.4.0（阶段三设计定稿）  
> 框架基准：Rust + Slint（歌词悬浮窗 + 设置窗口统一 Slint 声明式组件；底层接入 Win32 平台能力）  
> 上游文档：`docs/dev-stage-3/req.md`、`docs/dev-stage-2/req2.md`、`docs/dev-stage-1/dev.md`  
> 适用对象：开发人员（Dever）、评审人员与测试人员  
> 核心任务：托盘交互与重载链路修复、设置界面现代卡片化美化与输入规范、歌词实时预览与全量功能加固、go-musicfox 终端托管与配置集成

---

## 1. 设计原则与整体目标

1. **端到端体验一致（Native Desktop Experience）**：
   - 修复 Windows 任务栏右下角托盘弹出菜单被屏幕底部截断的边界问题，保证菜单顶部不超过鼠标光标且底部不溢出屏幕。
   - 托管 go-musicfox 的 Windows Terminal（WT）窗口，将其改造成无边框、无标题栏、无任务栏图标、防意外关闭且支持透明显隐切换的音乐伴侣挂件。
2. **现代美观与声明式统一（Fluent Card UI & Slint Architecture）**：
   - 全面重构设置界面布局为 **现代 Fluent 卡片式**（Grouped Cards），统一中性浅色底色、纯白卡片圆角容器、微边框、标准间距与标签对齐；
   - 依托 Slint 的属性双向绑定与声明式布局，在设置面板中直接实现低开销、即时跟随的歌词样式实时预览组件；
   - 输入框全面规范化为单行文本框（`LineEdit`），禁止使用不适合窗口尺寸与端口填写的拖拽手柄与微调按钮。
3. **真实反馈与确定性 IPC（Explicit Semantic Feedback）**：
   - 彻底解决设置面板「重载歌词」对主进程状态无感的问题；IPC 管道通信增加明确的调用结果判定与差异化 Toast 提示。
4. **全量功能可靠性确认（Full Functional Reliability）**：
   - 对迁移至 Slint 后的全部 14 项核心业务链路（配置加载、临时副本恢复、位置管道查询、防抖落盘、校验拦截、保存应用、重载反馈、字体枚举、未保存拦截、单实例激活等）实施严格的回归测试与加固。
5. **平滑演进与配置安全（Safe Evolution & Fault Tolerance）**：
   - 新增 `[wt]` 配置节，保持旧配置反序列化完全兼容（缺失时走 `Default`）；
   - 文件与目录探测采用严格的安全校验与多级 Fallback 机制，确保在各种环境变量与便携模式下均能正确定位。

---

## 2. 阶段规划与交付矩阵

```
┌────────────────────────────────────────────────────────────────────────┐
│ P0: 托盘交互与重载链路修复 (Tray Position / Reload IPC & Toast)         │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
┌──────────────────────────────────▼─────────────────────────────────────┐
│ P1: 设置界面现代卡片化美化与输入规范 (Fluent Cards / Colors / Inputs)     │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
┌──────────────────────────────────▼─────────────────────────────────────┐
│ P2: 歌词样式实时预览与全量功能加固 (Slint Preview / F01~F14 Testing)    │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
┌──────────────────────────────────▼─────────────────────────────────────┐
│ P3: go-musicfox WT 终端原生托管 (WT Process / Hook / Layered Alpha / Tray) │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
┌──────────────────────────────────▼─────────────────────────────────────┐
│ P4: 设置界面 go-musicfox 配置卡片与数据目录 (Config Section / RFD / DataDir)│
└────────────────────────────────────────────────────────────────────────┘
```

| 阶段 | 交付模块 | 关键能力 | 验证标准 |
|---|---|---|---|
| **P0** | 托盘与 IPC 修复 | 1. 托盘右键菜单 Y 轴锚点锁定（顶部 ≤ 鼠标 Y，底部 ≤ 屏幕高）<br>2. 设置窗口「重载歌词」精确返回主进程在线/离线 Toast<br>3. 托盘左键与右键命令重构 | 1. 1080p 屏幕右下角托盘右键菜单完整显示无截断<br>2. 主进程未运行时点击重载提示「主进程未运行...」；运行时提示「已发送重载通知」 |
| **P1** | 设置美化与输入规范 | 1. 界面采用浅灰底色 + 纯白圆角微边框卡片化布局<br>2. 宽/高改纯数字 `LineEdit`，非数字字符保存过滤，空值越界提示<br>3. 接收/发送端口改只读 `LineEdit` 并附灰色说明<br>4. 底部固定操作栏主次按钮高亮与模态弹窗美化 | 1. 界面具有清晰的现代 Fluent 层次感，各区块卡片分明<br>2. 宽/高输入框无拖动手柄与 ± 按钮，端口输入框不可编辑<br>3. 操作栏主按钮「💾 保存并应用」高亮突出 |
| **P2** | 实时预览与功能加固 | 1. 歌词样式实时预览区域（`#d4dce7` 背景，固定文字「天青色等烟雨」）<br>2. 预览调整不产生未保存脏标记<br>3. 全量功能检查与确认（F01~F14 逐项回归） | 1. 调整字号/颜色/字体/描边时预览区实时跟随，关闭无未保存弹窗<br>2. 14 项既有功能测试全部通过（位置拉取、防抖保存、校验拦截、损坏恢复等） |
| **P3** | WT 终端托管 | 1. 查找/启动 WT 运行 go-musicfox<br>2. WT 窗口样式注入（工具窗口、分层窗口、移除系统菜单/最大最小化、禁用关闭）<br>3. `SetWinEventHook` 拦截最小化为透明隐藏<br>4. 托盘单击切换显示（alpha=255）/ 隐藏（alpha=0）<br>5. 进程退出清理钩子并关闭 WT 窗口 | 1. 单击托盘：无 WT 则启动，有 WT 则切换透明度<br>2. 点击 WT 最小化按钮被拦截为透明隐藏且 ConPTY 持续运行<br>3. 退出 lyric 主进程时 WT 窗口同步关闭 |
| **P4** | go-musicfox 配置 | 1. `config.toml` 新增 `[wt]` 配置节<br>2. 设置页新增「🎵 go-musicfox」卡片，包含可执行文件路径 + 原生「浏览…」对话框<br>3. 「打开 go-musicfox 数据目录」按钮（4 级 fallback 精准定位 `DataDir()`）与当前路径提示 | 1. 点击「浏览…」可选择 `.exe` 并填入<br>2. 点击「打开数据目录」准确打开 `%LOCALAPPDATA%\go-musicfox` 或 `<MUSICFOX_ROOT>\data`<br>3. 修改路径保存后下次启动 WT 生效 |

---

## 3. 分阶段实施设计

---

### 🟢 P0 阶段：托盘交互与重载链路修复

#### 1. 阶段目标
- 修复 `tray-icon` 右键菜单在屏幕底部弹出时底部被任务栏或屏幕边缘截断的问题。
- 重构 IPC reload 管道通信机制，返回显式执行状态。
- 设置界面「重载歌词」根据管道通信结果展示差异化 Toast 提示。
- 托盘单击事件解耦（为 P3 WT 托管预留 `TrayCmd::ToggleWt`）。

#### 2. 涉及文件清单
- `src/tray/windows_tray.rs`：托盘菜单弹出位置计算、`TrayCmd` 命令枚举扩展。
- `src/pipe/mod.rs`：`reload::notify_reload` 签名调整为返回 `Result<(), ...>`。
- `src/platform/windows/pipe.rs`（或相关 platform 实现）：底层 Named Pipe 发送逻辑支持区分连接失败与发送成功。
- `src/settings/mod.rs`：设置窗口重载按钮回调处理与 Toast 触发。

#### 3. 详细设计与步骤

##### Step 1: 托盘菜单 Y 轴锚点位置约束
- **问题分析**：`tray-icon` 在 Windows 平台默认弹出菜单时以鼠标坐标为基准，但在底部任务栏右键时，OS 默认菜单弹出方向可能导致菜单向下延伸超出屏幕。
- **实施方案**：
  - 在弹出右键菜单前获取当前鼠标物理像素坐标 `(cursor_x, cursor_y)` 及当前屏幕工作区尺寸；
  - 约束菜单弹出锚点：保持 X 轴与鼠标对齐，Y 轴强制锚定在 `cursor_y`，并确保菜单顶部边界 ≤ `cursor_y`；若菜单预计底部 `cursor_y + menu_height > screen_height`，则向上调整或利用 Win32 `TrackPopupMenuEx` 的 `TPM_BOTTOMALIGN` / `TPM_VERPOSANIMATION` 特性保证菜单完整可见。
  - 对于 `tray-icon` crate，调用 `Menu::popup_at` 时传入修正后的物理坐标点。

##### Step 2: Reload 管道通信机制改造
- **现状**：`pipe::reload::notify_reload()` 为 `pub fn notify_reload()`，内部静默吞掉错误，设置界面无法感知主进程是否存在。
- **接口调整**：
  ```rust
  // src/pipe/mod.rs
  pub mod reload {
      pub fn notify_reload() -> Result<(), crate::error::AppError> {
          let sid = crate::protocol::get_session_id();
          let name = crate::protocol::platform_pipe_path("reload", sid);
          let msg = crate::protocol::IpcMessage::ReloadConfig.to_bytes();
          crate::platform::current().send(&name, &msg, 1000)
      }
  }
  ```
- **平台层支持**：确保 `crate::platform::current().send` 在管道不存在（`ERROR_FILE_NOT_FOUND` / 连接超时）时返回具体的 `AppError::PipeConnectionFailed` 或 `AppError::Io`。

##### Step 3: 设置界面重载交互与 Toast 呈现
- **设置界面回调处理**：
  在 `src/settings/mod.rs` 的 `reload-clicked` 事件处理中：
  ```rust
  // 伪逻辑
  app.on_reload_clicked(move || {
      match crate::pipe::reload::notify_reload() {
          Ok(_) => {
              show_toast("已发送重载通知", Duration::from_secs(3));
          }
          Err(_) => {
              show_toast("主进程未运行，无法重载（重启 lyric-for-musicfox 后生效）", Duration::from_secs(3));
          }
      }
  });
  ```
- **文案约束**：
  - 「重载歌词」成功 → `已发送重载通知`
  - 「重载歌词」失败 → `主进程未运行，无法重载（重启 lyric-for-musicfox 后生效）`
  - 「保存并应用」成功 → `配置已保存并已实时生效`

##### Step 4: 托盘事件与命令枚举扩展
- 在 `TrayCmd` 中扩展枚举项：
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum TrayCmd {
      OpenSettings,
      ReloadLyrics,
      ToggleWt,
      Quit,
  }
  ```
- 托盘左键单击分发：改为发送 `TrayCmd::ToggleWt`（P0 阶段主进程收到后若未实现 WT 则记录 debug 日志，P3 正式接入）。
- 托盘右键菜单增加「🎵 go-musicfox」项，点击同样分发 `TrayCmd::ToggleWt`。

#### 4. 验收要点
- [ ] 1080p 屏幕右下角点击托盘右键，菜单完整弹出，菜单最顶部不超过鼠标位置，最底部的「退出」项清晰可见。
- [ ] 未启动主进程时，独立启动 `lyric-for-musicfox.exe --settings`，点击「🔄 重载歌词」，右下角弹出 Toast「主进程未运行，无法重载（重启 lyric-for-musicfox 后生效）」，并在 3 秒后平滑消失。
- [ ] 启动主进程后，在设置界面点击「🔄 重载歌词」，弹出 Toast「已发送重载通知」，主进程成功重新加载配置。

---

### 🟢 P1 阶段：设置界面现代卡片化美化与输入规范化

#### 1. 阶段目标
- 全面重构 `ui/settings.slint` 界面视觉，采用 **现代 Fluent 卡片化（Grouped Cards）** 风格。
- 引入统一的调色板、圆角容器、标签对齐规范与底部操作栏高亮样式。
- 改造设置面板中的宽/高输入框与端口输入框，消除原有控件的拖动手柄和 ± 调节按钮。
- 窗口端口字段设为只读并添加说明提示。

#### 2. 涉及文件清单
- `ui/settings.slint`：卡片化布局重构（定义 `SettingsCard` / `CardHeader` / `FormRow` 结构，应用现代配色与圆角）。
- `src/settings/mod.rs`：表单字段与 Slint 属性绑定、非数字字符过滤处理。
- `src/settings/validate.rs`：纯数字解析与空值兜底校验。

#### 3. 详细设计与步骤

##### Step 1: 视觉设计规范与调色板定义
- **色彩规范**：
  - 窗口背景色（Window Background）：`#f3f4f6`（柔和中性浅灰）
  - 卡片背景色（Card Background）：`#ffffff`（纯白）
  - 卡片边框色（Card Border）：`#e5e7eb`（1px 细腻微边框）
  - 主色调（Primary Accent）：`#2563eb`（现代蓝，用于主按钮高亮、选中高亮）
  - 次级按钮背景：`#f9fafb`（带 `#d1d5db` 边框）
  - 标签文本色：`#1f2937`（深灰，字重 500）
  - 提示文本色：`#6b7280`（中性灰，11~12px）
  - 错误/警示色：`#dc2626`（警示红）
- **尺寸与间距**：
  - 窗口尺寸：540px × 700px；
  - 外层内边距：`padding: 16px`；卡片垂直间距：`spacing: 12px`；
  - 卡片内部内边距：`padding: 14px`；卡片内行间距：`spacing: 10px`；
  - 卡片圆角：`border-radius: 8px`；
  - 标签列宽度：固定 `90px` 严格左对齐。

##### Step 2: Slint 卡片化容器封装（`ui/settings.slint`）
在 Slint 中重构卡片布局，将每个模块封装为独立卡片容器：
```slint
// 卡片容器伪结构
Rectangle {
    background: #ffffff;
    border-radius: 8px;
    border-width: 1px;
    border-color: #e5e7eb;

    VerticalLayout {
        padding: 14px;
        spacing: 10px;

        // 卡片标题
        Text {
            text: "🪟 窗口与定位";
            font-size: 14px;
            font-weight: 700;
            color: #111827;
        }

        // 行级表单项 (各行统一排版)
        ...
    }
}
```

##### Step 3: 输入控件规范化（宽/高与端口）
- **窗口尺寸（宽/高）**：
  - 使用标准 `LineEdit`，移除任何带上下微调手柄的控件；
  - 占位与布局保持紧凑工整；
- **网络端口（接收/发送）**：
  - 设置 `enabled: false`，呈现只读灰度状态；
  - 字段后附说明文本：`Text { text: "端口变更需重启主进程生效；此处只读"; color: #6b7280; font-size: 11px; }`。
- **Rust 侧非数字字符过滤**：
  - 在保存解析时按整数解析，自动剔除非数字字符；空字符串解析为 `0` 并触发 `validate_width` / `validate_height` 错误红字。

##### Step 4: 底部操作栏与模态弹窗美化
- **操作栏（Bottom Bar）**：
  - 底部固定高度（60px），顶部具备 `1px` 浅灰色分隔边框（`#e5e7eb`）；
  - 主操作按钮「💾 保存并应用」设置为主色高亮样式；
  - 次要按钮「🔄 重载歌词」「📁 打开配置目录」使用标准次级样式；
  - 错误提示信息在操作栏右侧垂直居中展示，红字醒目。
- **模态弹窗（Modals）**：
  - 遮罩层使用 `#66000000` 半透明暗黑蒙层；
  - 居中卡片采用 `background: white`、`border-radius: 10px`、精致内边距 `20px`；
  - 按钮组按「主要 / 次要 / 取消」排版对齐。

#### 4. 验收要点
- [ ] 打开设置窗口，整体呈现清爽现代的浅灰底色 + 白色圆角微边框卡片风格，无杂乱平铺感。
- [ ] 四大卡片（窗口与定位、歌词与样式、系统与网络、go-musicfox）分区明确，各行标签对齐工整。
- [ ] 宽/高输入框无拖动手柄与上下加减箭头；端口输入框呈只读灰色，无法编辑。
- [ ] 底部操作栏「💾 保存并应用」主按钮视觉突出。

---

### 🟢 P2 阶段：歌词样式实时预览与全量功能检查加固

#### 1. 阶段目标
- 在「🎨 歌词与样式」卡片内底部嵌入实时歌词样式预览区域。
- 保证预览区域与表单脏检查机制完全隔离。
- 按照 **F01 ~ F14** 功能检查矩阵，对 Slint 设置进程进行全链路回归测试与代码加固。

#### 2. 涉及文件清单
- `ui/settings.slint`：嵌入歌词预览子组件。
- `src/settings/mod.rs`：预览样式属性映射、全量事件回调与状态机加固。
- `src/settings/form.rs`：脏标记隔离与防抖暂存逻辑加固。
- `src/settings/validate.rs`：校验规则完善。

#### 3. 详细设计与步骤

##### Step 1: 歌词样式预览组件集成（`ui/settings.slint`）
- **规格参数**：
  - **容器**：高度固定 `80px`，宽度填满卡片，背景固定 `#d4dce7`（RGB: 212, 220, 231），圆角 `6px`，`clip: true`；
  - **预览文本**：固定内容为 `"天青色等烟雨"`，水平垂直居中；
  - **实时联动属性**：
    - 字体族：绑定 `root.font-current-value`；
    - 字号：绑定 `root.font-size-pt * 1pt`；
    - 加粗/斜体：绑定 `root.font-bold ? 700 : 400` 与 `root.font-italic`；
    - 文本颜色：实时解析 `root.font-color-text`（不合法时回退安全默认色）；
    - 描边：当 `root.outline-enabled` 为真时，采用 8 向偏移 `Text` 元素渲染描边，描边颜色与宽度绑定对应表单属性。
- **脏标记隔离**：
  - 预览区所有属性变更直接跟随 UI 状态树响应式刷新，不调用 `form.mark_dirty()`；
  - 用户单纯在预览区观察样式不触发未保存弹窗。

##### Step 2: 全量功能检查与测试加固（F01 ~ F14）
针对需求文档 §8 的 14 项功能矩阵进行代码审查与加固：
1. **F01/F02 配置初始化**：验证 `config.toml` 缺失时使用 `Config::default()` 填充表单，无 Panic；
2. **F03 临时副本恢复**：验证存在 `config.toml.tmp` 时正确恢复草稿数据，且当内容与 `config.toml` 不一致时标记 `dirty = true`；
3. **F04 损坏恢复**：验证 `config.toml` 格式损坏时弹出「配置解析失败」模态框，点击「使用默认配置重置」恢复默认；
4. **F05/F06 位置管道联动**：验证启动时异步向 `pos` 管道请求坐标，在线回填最新 X/Y；离线（1s 超时）降级为历史配置并显示灰色提示；
5. **F07 防抖暂存**：修改字段 500ms 后自动调用 `save_tmp` 写入 `.tmp`；
6. **F08 校验拦截**：输入非法值点击保存时被拦截，红字提示正确显示；
7. **F09 保存并应用**：原子写 `config.toml` + 发送 `reload` 管道通知 + 弹出 Toast（3s 消失）；
8. **F10 独立重载反馈**：主进程在线弹出「已发送重载通知」，离线弹出「主进程未运行...」；
9. **F11 字体族枚举**：验证 GDI 字体枚举正常填充 `ComboBox` 下拉模型；
10. **F12 打开配置目录**：调用系统文件管理器打开 `%APPDATA%\lyric-for-musicfox\`；
11. **F13 未保存关闭拦截**：表单处于脏状态时关闭窗口弹出确认框，支持「保存」「丢弃」「取消」；
12. **F14 单实例与激活**：多实例启动时通过 `presence` 管道激活旧窗口。

#### 4. 验收要点
- [ ] 预览区域背景呈现浅蓝灰（`#d4dce7`），文字「天青色等烟雨」居中显示，调整字号、颜色、字体族、描边时画面即刻变化。
- [ ] 打开设置窗口未做任何修改直接关闭，窗口正常退出，不弹出未保存提示。
- [ ] 14 项功能检查项（F01~F14）逐项通过回归测试。

---

### 🟢 P3 阶段：go-musicfox WT 终端窗口原生托管

#### 1. 阶段目标
- 在主进程中实现 Windows Terminal（`wt.exe`）窗口的查找、启动、样式改造与透明显隐控制。
- 接管托盘单击事件：控制 WT 窗口在显示与透明隐藏之间无缝切换。
- 拦截 WT 窗口的最小化操作，转化为透明隐藏，保持 ConPTY 终端后台持续运行。
- 主进程退出时联动关闭托管的 WT 窗口并释放事件钩子。

#### 2. 涉及文件清单
- `src/config/mod.rs`：新增 `WtConfig` 结构体及默认字段。
- `src/services/wt.rs`（新建）：WT 窗口生命周期管理、启动逻辑、样式注入与 WinEventHook 回调分发。
- `src/platform/windows/wt.rs`（新建）：Win32 原生 API 封装（`FindWindowW`、`SetWindowLongPtrW`、`SetLayeredWindowAttributes`、`SetWinEventHook`、`UnhookWinEvent` 等）。
- `src/tray/windows_tray.rs`：单击托盘分发 `TrayCmd::ToggleWt`。
- `src/main.rs`：启动后延迟 1s 初始化 WT 托管，退出时调用清理。

#### 3. 详细设计与步骤

##### Step 1: 配置定义（`src/config/mod.rs`）
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WtConfig {
    #[serde(default = "default_wt_musicfox_path")]
    pub musicfox_path: String,
    #[serde(default = "default_wt_app_dir")]
    pub app_dir: String,
    #[serde(default = "default_wt_title")]
    pub title: String,
}

fn default_wt_musicfox_path() -> String {
    "C:\\Users\\xx\\app\\musicfox\\musicfox.exe".into()
}
fn default_wt_app_dir() -> String {
    "C:\\Users\\xx\\app".into()
}
fn default_wt_title() -> String {
    "MusicFoxTerminal".into()
}

impl Default for WtConfig {
    fn default() -> Self {
        Self {
            musicfox_path: default_wt_musicfox_path(),
            app_dir: default_wt_app_dir(),
            title: default_wt_title(),
        }
    }
}
```
在 `Config` 中加入 `pub wt: WtConfig`，默认值为 `#[serde(default)]`。

##### Step 2: WT 查找与启动（`src/services/wt.rs`）
- **窗口查找**：
  - 调用 `FindWindowW(None, wt_title)` 匹配标题为 `MusicFoxTerminal` 的顶层窗口；
  - 校验该窗口所属进程名是否为 `WindowsTerminal.exe`。
- **启动进程**：
  - 若窗口不存在，使用 `std::process::Command` 启动：
    ```text
    wt.exe -w new -d "<app_dir>" --title "<title>" "<musicfox_path>"
    ```
  - 启动后设置 1 秒定时器执行样式初始化与钩子安装。

##### Step 3: WT 窗口样式改造与菜单裁剪
当获取到 WT 的 `HWND` 时，执行原生样式改造：
1. **注入扩展样式**：
   - 增加 `WS_EX_TOOLWINDOW`（使其脱离 Windows 任务栏，不占任务栏图标）；
   - 增加 `WS_EX_LAYERED`（支持分层透明度控制）；
2. **移除标准窗口样式**：
   - 移除 `WS_CAPTION` 与 `WS_SYSMENU`（去除原有标题栏与系统菜单边框）；
   - 移除 `WS_MAXIMIZEBOX` 与 `WS_MINIMIZEBOX`（去除最大化与最小化按钮）；
3. **禁用关闭功能**：
   - 获取系统菜单 `GetSystemMenu(hwnd, FALSE)`，对 `SC_CLOSE` 调用 `EnableMenuItem(..., MF_GRAYED | MF_BYCOMMAND)`，防止用户误触关闭终止播放器；
4. **刷新窗口帧**：调用 `SetWindowPos` 携带 `SWP_FRAMECHANGED` 使样式即刻生效。

##### Step 4: 显隐状态机与 Alpha 切换
- **状态判定**：维护 WT 当前是否处于隐藏状态（通过查询 Layered 属性的 alpha 值或内部布尔标记）；
- **显示（ShowWT）**：
  - 调用 `SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA)`；
  - 调用 `SetForegroundWindow(hwnd)` 激活置顶前台；
- **隐藏（HideWT）**：
  - 调用 `SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA)`（完全透明）；
  - **严禁**调用 `ShowWindow(hwnd, SW_HIDE)`，必须保持 `SW_SHOWNORMAL` 状态以保证 Windows ConPTY 子系统正常驱动 go-musicfox 歌词与音频解码循环。

##### Step 5: 最小化事件拦截（WinEventHook）
- **安装钩子**：
  - 调用 `SetWinEventHook` 监听 `EVENT_SYSTEM_MINIMIZESTART (0x0016)`；
- **非阻塞脱离分发**：
  - 钩子回调捕获到事件后，通过消息队列或异步线程调用 `HideWT`，恢复窗口正常形态并设置 alpha=0。

##### Step 6: 退出与资源清理
- 主进程收到退出信号时：
  1. 调用 `UnhookWinEvent` 注销事件钩子；
  2. 若 WT 窗口句柄有效，向其发送 `WM_CLOSE` 消息促使优雅退出。

#### 4. 验收要点
- [ ] 单击托盘图标：若 WT 未运行，自动拉起 Windows Terminal 并运行 go-musicfox，1 秒后窗口自动去除标题栏和任务栏图标。
- [ ] 再次单击托盘图标：WT 窗口瞬间透明隐藏（音频继续播放）；再次单击：WT 窗口恢复可见并置前。
- [ ] 点击最小化快捷操作，窗口被拦截为透明隐藏。
- [ ] 右键托盘退出应用，WT 终端窗口随之正常关闭。

---

### 🟢 P4 阶段：设置界面 go-musicfox 配置卡片与数据目录集成

#### 1. 阶段目标
- 在设置面板中新增「🎵 go-musicfox」卡片，提供可执行文件路径配置与原生文件选择。
- 实现精准复刻 go-musicfox 源码的 `DataDir()` 数据目录解析算法，并集成「打开 go-musicfox 数据目录」按钮。
- 配置项正常落盘至 `config.toml` 并与主进程解耦（下次启动 WT 生效）。

#### 2. 涉及文件清单
- `Cargo.toml`：引入 `rfd`（Rust File Dialogs）crate。
- `ui/settings.slint`：新增「🎵 go-musicfox」卡片组件与相关回调、属性。
- `src/settings/mod.rs`：路径绑定、文件对话框触发、数据目录打开与错误 Toast 调度。
- `src/settings/validate.rs`：`wt_musicfox_path` 路径存在性与非空校验。
- `src/services/musicfox.rs`（或相关模块）：`resolve_musicfox_data_dir()` 4 级解析算法。

#### 3. 详细设计与步骤

##### Step 1: go-musicfox 数据目录解析算法实现
严格复刻 go-musicfox `utils/app/app.go` 中的路径解析规则：
```rust
pub fn resolve_musicfox_data_dir() -> Option<std::path::PathBuf> {
    // 1. portable 模式：MUSICFOX_ROOT 优先，数据位于 <MUSICFOX_ROOT>/data
    if let Ok(root) = std::env::var("MUSICFOX_ROOT") {
        if !root.trim().is_empty() {
            return Some(std::path::PathBuf::from(root.trim()).join("data"));
        }
    }
    // 2. XDG 模式：XDG_DATA_HOME/go-musicfox
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        if !xdg_data.trim().is_empty() {
            return Some(std::path::PathBuf::from(xdg_data.trim()).join("go-musicfox"));
        }
    }
    // 3. Windows 默认：LOCALAPPDATA\go-musicfox (注意：不是 Roaming APPDATA)
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            if !local.trim().is_empty() {
                return Some(std::path::PathBuf::from(local.trim()).join("go-musicfox"));
            }
        }
        // 4. Windows 兜底：USERPROFILE\AppData\Local\go-musicfox
        if let Ok(profile) = std::env::var("USERPROFILE") {
            if !profile.trim().is_empty() {
                return Some(
                    std::path::PathBuf::from(profile.trim())
                        .join("AppData")
                        .join("Local")
                        .join("go-musicfox"),
                );
            }
        }
    }
    None
}
```

##### Step 2: Slint 设置界面卡片布局（`ui/settings.slint`）
在「⚙️ 系统与网络」卡片之后追加「🎵 go-musicfox」卡片（遵循 P1 卡片化规范）：
```slint
// 🎵 go-musicfox 卡片容器
Rectangle {
    background: #ffffff;
    border-radius: 8px;
    border-width: 1px;
    border-color: #e5e7eb;

    VerticalLayout {
        padding: 14px;
        spacing: 10px;

        Text {
            text: "🎵 go-musicfox";
            font-size: 14px;
            font-weight: 700;
            color: #111827;
        }

        HorizontalLayout {
            spacing: 8px;
            Text { text: "程序路径"; vertical-alignment: center; width: 90px; }
            LineEdit {
                text <=> root.wt-musicfox-path-text;
                edited => { root.changed(); }
                horizontal-stretch: 1;
            }
            Button {
                text: "浏览…";
                clicked => { root.browse-musicfox-clicked(); }
            }
        }

        HorizontalLayout {
            spacing: 8px;
            Text { width: 90px; } // 对齐占位
            Button {
                text: "📂 打开 go-musicfox 数据目录";
                clicked => { root.open-musicfox-data-clicked(); }
            }
            Text {
                text: root.musicfox-data-dir-hint;
                color: #6b7280;
                font-size: 11px;
                vertical-alignment: center;
            }
        }
    }
}
```

##### Step 3: 事件与交互响应（`src/settings/mod.rs`）
1. **「浏览…」按钮回调**：
   ```rust
   if let Some(path) = rfd::FileDialog::new()
       .add_filter("Executable", &["exe"])
       .pick_file() 
   {
       app.set_wt_musicfox_path_text(path.to_string_lossy().to_string().into());
       form.mark_dirty();
   }
   ```
2. **「打开数据目录」按钮回调**：
   - 解析有效路径且目录存在时，调用资源管理器打开；
   - 解析失败弹出 Toast「无法定位 go-musicfox 数据目录」（3s 自动消失）；
3. **只读路径提示**：
   - 界面启动时调用 `resolve_musicfox_data_dir()` 回填 `musicfox-data-dir-hint` 属性。

##### Step 4: 校验与配置持久化
- `src/settings/validate.rs` 增加 `validate_musicfox_path`（非空与存在性校验）；
- 点击「💾 保存并应用」时，`wt_musicfox_path` 随其他字段原子落盘至 `config.toml` 的 `[wt]` 节；
- 运行时热生效：不打断当前正在运行的 WT，下次启动 WT 实例时读取最新路径。

#### 4. 验收要点
- [ ] 设置窗口中呈现「🎵 go-musicfox」卡片，包含程序路径输入框、浏览按钮、打开数据目录按钮及灰色只读路径提示。
- [ ] 点击「浏览…」，弹出原生 Windows 文件选择框，选中后路径正确填入文本框。
- [ ] 点击「📂 打开 go-musicfox 数据目录」，Windows 资源管理器准确打开 `%LOCALAPPDATA%\go-musicfox` 并能看见 `cookie` 与 `musicfox.db`。
- [ ] 填入不存在的文件路径点击保存，界面显示红字校验错误。
- [ ] 保存成功后检查 `%APPDATA%\lyric-for-musicfox\config.toml`，包含正确的 `[wt]` 配置节。

---

## 4. 平台抽象层与 Win32 原生接口规范

针对阶段三引入的系统级特性，在 `src/platform/` 中统一隔离 Win32 API：

| 模块 | 功能函数 | 底层 Windows API / 机制 | 说明 |
|---|---|---|---|
| **Tray Position** | `calculate_safe_menu_pos` | `GetCursorPos`, `GetMonitorInfoW` | 约束托盘菜单顶部 ≤ 鼠标 Y，底部 ≤ 屏幕物理像素高度 |
| **WT Window** | `find_wt_window` | `FindWindowW`, `GetWindowThreadProcessId` | 根据标题匹配 WT 宿主窗口 |
| **WT Style** | `apply_wt_hosted_style` | `GetWindowLongPtrW`, `SetWindowLongPtrW`, `SetWindowPos`, `GetSystemMenu`, `EnableMenuItem` | 注入 `WS_EX_TOOLWINDOW \| WS_EX_LAYERED`，去除 `WS_CAPTION \| WS_SYSMENU \| WS_MAXIMIZEBOX \| WS_MINIMIZEBOX`，禁用 `SC_CLOSE` |
| **WT Alpha** | `set_window_alpha` | `SetLayeredWindowAttributes` | `alpha = 255`（显示并激活），`alpha = 0`（完全透明隐藏） |
| **WT Hook** | `install_minimize_hook`, `uninstall_hook` | `SetWinEventHook`, `UnhookWinEvent` | 监听 `EVENT_SYSTEM_MINIMIZESTART (0x0016)` 事件 |
| **File Manager** | `open_directory` | `ShellExecuteW` 或 `Command::new("explorer")` | 打开目标数据目录 |

---

## 5. 异常处理与边界矩阵

| 场景 | 异常/边界条件 | 系统应对策略 |
|---|---|---|
| **托盘菜单弹出** | 鼠标位于屏幕绝对底部或右下角角落 | 强制菜单 Y 轴不超过鼠标 Y，向上或向下对齐工作区内，禁止溢出屏幕 |
| **重载歌词** | 主进程完全未启动，设置进程点击「重载歌词」 | 管道 Connect 失败后立即捕获，Toast 准确提示「主进程未运行，无法重载...」，3秒自动消失，不阻塞界面 |
| **尺寸输入** | 用户输入非法字符（如 `abc`、`12.5`、`-50`） | 过滤非法字符，空值按 `0` 处理，保存校验触发越界红字，不引起程序 Panic |
| **样式预览** | 用户输入未完成的 Hex 颜色（如 `#ff`） | 预览层安全捕获，保持回退色渲染，不引起 Slint 属性崩溃 |
| **WT 启动** | 配置的 `musicfox_path` 路径不存在 | 设置界面保存时标红拦截；托盘单击启动失败时记录 warn 日志并保持应用稳定 |
| **WT 最小化** | 用户快捷键触发 WT 最小化 | WinEventHook 捕获后在独立队列触发 `HideWT`，恢复窗口非最小化态并置透明度为 0 |
| **数据目录** | 环境变量缺失（如非标准精简系统） | 走 4 级 Fallback，终极失败时 Toast 提示「无法定位 go-musicfox 数据目录」 |

---

## 6. 测试与回归验证清单

### 1. 单元测试
- [ ] `config::wt` 配置序列化与反序列化测试（包含字段缺失默认值验证）。
- [ ] `validate_width` / `validate_height` / `validate_musicfox_path` 边界值与合法性验证。
- [ ] `resolve_musicfox_data_dir` 在模拟不同环境变量（`MUSICFOX_ROOT`、`XDG_DATA_HOME`、`LOCALAPPDATA`）下的路径解析准确性测试。

### 2. 集成与全量功能回归测试 (F01 ~ F14)
- [ ] **托盘右键测试**：在屏幕底部、顶部、中间分别右键，观察菜单弹出方向与边界。
- [ ] **重载通信测试**：分别在主进程存活和主进程退出两种状态下测试设置面板重载按钮的反馈文案。
- [ ] **现代卡片美化与输入测试**：
  - 验证界面呈现浅灰底色 + 纯白圆角微边框卡片风格；
  - 验证宽高输入框无微调按钮；
  - 验证端口输入框不可修改且有灰色说明；
  - 验证底部操作栏「💾 保存并应用」主按钮高亮。
- [ ] **实时预览测试**：
  - 验证预览框背景为 `#d4dce7`，文字为「天青色等烟雨」，修改字号/颜色/字体族/加粗/斜体/描边时预览即时变化；
  - 验证仅查看预览不修改字段时关闭窗口无未保存弹窗。
- [ ] **WT 托管全生命周期测试**：
  - 单击托盘自动拉起 WT，无控制台标题栏与任务栏图标；
  - 单击托盘在可见与透明隐藏间切换，音频播放不中断；
  - 最小化拦截为透明；
  - 退出主进程同步退出 WT。
- [ ] **go-musicfox 配置卡片测试**：
  - 使用「浏览…」选择 `musicfox.exe`；
  - 点击「打开 go-musicfox 数据目录」验证是否能正确打开对应文件夹并定位到 cookie 等数据文件。
