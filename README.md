# lyric-for-musicfox

<p align="center">
  <strong>专为 <a href="https://github.com/go-musicfox/go-musicfox">go-musicfox</a> 打造的轻量级 Windows 桌面悬浮歌词与终端原生托管扩展</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-Windows%2010%2F11-blue" alt="Platform">
  <img src="https://img.shields.io/badge/Language-Rust%202021-orange" alt="Language">
  <img src="https://img.shields.io/badge/UI-Slint%20(Skia%20Software)-teal" alt="UI Framework">
  <img src="https://img.shields.io/badge/License-MIT-green" alt="License">
</p>

---

## 核心特性

- **透明悬浮歌词**：无边框置顶、8 方向描边、超宽自动滚动、支持鼠标拖拽与位置锁定，适配多显示器 DPI。
- **WT 终端托管**：托盘单击快捷唤起/隐藏 go-musicfox，自动注入无边框小部件样式并拦截最小化。
- **可视化设置**：现代卡片化界面、实时样式渲染预览、Win32 GDI 字体列表秒级枚举。
- **极低资源占用**：稳态内存约 25MB，空闲 CPU 0.0%，基于 Windows 命名管道的轻量异步通信。

---

## 构建与安装

### 环境要求
- **系统**：Windows 10 (1809+) / Windows 11 (x86_64)
- **工具链**：Rust 1.75+（`x86_64-pc-windows-msvc`）

### 常用命令

```bash
# 运行单元测试
cargo test

# 编译 Windows Release 版本 (MSVC + Skia 软件渲染)
./build.sh windows

# （可选）编译打好 UDP 歌词补丁的 musicfox.exe
./build.sh musicfox
```

产物路径：`target/x86_64-pc-windows-msvc/release/lyric-for-musicfox.exe`

---

## 使用指南

### 1. 运行方式

```bash
# 启动歌词主程序（托盘驻留 + 悬浮窗）
lyric-for-musicfox.exe

# 打开设置面板
lyric-for-musicfox.exe --settings

# 性能测试模式（输出 UDP 与渲染延迟）
lyric-for-musicfox.exe --benchmark
```

### 2. 托盘交互
- **左键单击**：无终端时启动并注入样式，有终端时在**完全显示（置前）**与**透明隐藏**间切换。
- **右键单击**：弹出菜单（配置 / 重载歌词 / 退出）。

### 3. go-musicfox 联动
1. 运行 `./musicfox-patch/build_musicfox_with_plugin.sh` 编译支持 UDP 歌词广播的 `musicfox.exe`；
2. 在设置面板「go-musicfox」卡片中填入或浏览选择该 `musicfox.exe` 路径。

---

## 配置文件说明

配置文件路径：`%APPDATA%\lyric-for-musicfox\config.toml`

```toml
[window]
width = 800               # 悬浮窗宽度 (DIP, 100~4000)
height = 80               # 悬浮窗高度 (DIP, 20~500)
pos_x = 560               # 物理坐标 X (留空则居中)
pos_y = 120               # 物理坐标 Y
stay_on_top = true        # 始终置顶
frame_less = true         # 无边框透明
locked = false            # 锁定位置 (不可拖拽)

[lyric_style]
font_family = "Microsoft YaHei"  # 字体家族
font_size = 24.0                 # 字号 (pt, 6~200)
font_bold = false                # 加粗
font_italic = false              # 斜体
font_color = "#ffffff"           # 文本颜色 (#RRGGBB)
font_outline_color = "#000000"   # 描边颜色
font_outline_width = 1           # 描边宽度 (0~16)

[system]
receive_port = 16501      # UDP 接收端口
send_port = 16502         # 指令出站端口
log_enabled = false       # 磁盘日志开关

[wt]
musicfox_path = "C:\\Tools\\musicfox\\musicfox.exe"  # musicfox 可执行文件路径
app_dir = ""                                         # 终端启动目录
title = "MusicFoxTerminal"                           # 终端窗口标题
```

---

## 开源协议

本项目基于 **[MIT License](LICENSE)** 授权开源。
