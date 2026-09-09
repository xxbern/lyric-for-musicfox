# 阶段 3 无用代码与依赖全面扫描报告

## 一、 扫描概述

为了保障项目架构的整洁性、性能及可维护性，本次扫描针对 `lyric-for-musicfox` 整个项目进行了深度的无用代码（Dead Code）、废弃逻辑、冗余资源及依赖项（Cargo Dependencies）全面盘点。

- **扫描对象**：
  1. Rust 后端源码 (`src/`) 及测试代码 (`tests/`)
  2. Services 服务代理层与 Platform Trait 抽象层
  3. Slint 前端 UI 组件 (`ui/*.slint`)
  4. Cargo 配置文件 (`Cargo.toml`) 与构建脚本 (`build.rs`, `build.sh`)
  5. 静态资源目录 (`assets/`) 及辅助补丁脚本 (`musicfox-patch/`)

---

## 二、 无用代码 (Dead Code) 与未调用符号清单

### 1. Services 服务层

| 符号 / 文件 | 类型 | 状态与问题说明 |
| :--- | :--- | :--- |
| `TrayService` (`src/services/tray.rs`) | 结构体 & 全套方法 | **全模块死代码**。文件明确标注为临时保留废弃代码。目前所有托盘逻辑均由 `crate::tray`（平台 trait 后端）直接承载，`TrayService` 上的 `new`, `init_tray`, `destroy_tray`, `update_flash` 均无任何调用点。 |
| `LyricUdpService::stop_listening` (`src/services/udp.rs`) | 实例方法 | **死方法**。方法体为空，且项目中没有任何地方调用该方法。 |
| `LyricUdpService::ctx` (`src/services/udp.rs`) | 结构体字段 | **未读取字段**。仅带有 `#[allow(dead_code)]` 标注，未被逻辑使用。 |
| `FontService::ctx` (`src/services/font.rs`) | 结构体字段 | **未读取字段**。带 `#[allow(dead_code)]` 标注，结构体上的 `resolve_family` 和 `get_font_bytes` 均为独立顶层函数的简单包装。 |
| `ConfigService::save` & `reload_from_disk` (`src/services/config.rs`) | 实例方法 | **未调用方法**。`ConfigService` 仅静态关联函数 `load_or_default()` 被调用，其实例方法 `save` 与 `reload_from_disk` 在任何地方均未被使用。 |
| `MonitorRect::height` (`src/services/monitor.rs`) | 实例方法 | **未调用方法**。`MonitorRect::width` 在居中计算中使用，但 `height` 从未被调用。 |
| `WindowStyleService::is_locked` (`src/services/style.rs`) | 实例方法 | **未调用方法**。该窗口锁定状态查询方法在生产及测试中均未被调用。 |

### 2. Platform 平台抽象层

| 符号 / 文件 | 类型 | 状态与问题说明 |
| :--- | :--- | :--- |
| `PlatformWindowStyle::has_transparent_style` (`src/platform/trait.rs`, `windows/mod.rs`, `stub/mod.rs`) | Trait 方法 & 平台实现 | **未调用 Trait 方法**。在 `PlatformWindowStyle` 及其 Windows 与 Stub 实现中均定义了 `has_transparent_style`，但在整个业务逻辑与 UI 渲染流程中无任何调用点。 |

### 3. 渲染与状态机模块

| 符号 / 文件 | 类型 | 状态与问题说明 |
| :--- | :--- | :--- |
| `RenderCache::playing_snapshot` (`src/window/render_cache.rs`) | 实例方法 | **未调用方法**。`RenderCache` 结构体内实现了 `playing_snapshot(&self)`，但在 `window/mod.rs` 及外部模块中均未调用该方法。 |

### 4. 错误处理模块 (AppError)

| 符号 / 文件 | 类型 | 状态与问题说明 |
| :--- | :--- | :--- |
| `AppError::SettingsUnavailable` (`src/error.rs`) | 枚举变体 | **冗余 Error 变体**。除了在 `exit_code` 和 `Display` 格式化 `match` 中保留外，项目中没有任何代码构造或返回 `AppError::SettingsUnavailable`。 |

### 5. 命令控制模块 (src/command/)

| 符号 / 文件 | 类型 | 状态与问题说明 |
| :--- | :--- | :--- |
| `Command` 枚举 & `send_command` (`src/command/mod.rs`, `src/command/udp_send.rs`) | 结构/函数 | **仅测试引用的功能模块**。`send_command` 及其操作指令枚举仅在 `tests/command_test.rs` 自动化测试中被调用。主应用 `src/main.rs` 和设置窗口均未建立 UDP 反向控制功能，属于预留/未上线模块。 |

---

## 三、 静态资源与构建配置扫描

1. **`assets/icon.rc` (静态资源遗留文件)**:
   - **问题说明**: `build.rs` 当前已重构为在 `$OUT_DIR` 下根据 `CARGO_PKG_VERSION` 动态生成包含最新版本号的 `icon.rc` 并进行编译 (`embed_resource::compile(&rc_path, ...)`）。
   - **结论**: 根目录下的静态 `assets/icon.rc` 已不再被构建脚本读取，属于硬编码时代的过时遗留文件。

2. **`musicfox-patch/go-musicfox` 目录状态**:
   - **问题说明**: `.gitignore` 中配置了 `/musicfox-patch/go-musicfox/` 排除规则，但根目录下依然保留了完备的源码镜像与本地编译产物。如果是为了 `build_musicfox_with_plugin.sh` 脚本使用，建议确认是否需要提交或仅保持纯净克隆。

---

## 四、 Cargo 依赖项 (Cargo.toml) 审查

经过全量 Grep 及 Symbol 引用交叉对比，`Cargo.toml` 中声明的所有直接依赖项均被实际代码引用：

| 依赖 Crate | 声明版本 | 使用模块 / 场景 | 状态判定 |
| :--- | :--- | :--- | :--- |
| `clap` | 4 (derive) | `src/cli.rs`, `src/main.rs` (命令行参数解析) | ✅ 正常使用 |
| `serde` / `serde_json` | 1 | `src/config/`, `src/lyric/udp.rs`, `src/command/` | ✅ 正常使用 |
| `toml` | 0.8 | `src/config/load.rs`, `src/config/save.rs` | ✅ 正常使用 |
| `thiserror` | 1 | `src/error.rs` (统一错误处理) | ✅ 正常使用 |
| `dirs` | 5 | `src/path.rs` (平台数据/配置目录解析) | ✅ 正常使用 |
| `log` | 0.4 | `src/logger.rs` 及全项目日志打印 | ✅ 正常使用 |
| `crossbeam-channel` | 0.5 | `src/logger.rs`, `src/event_bus/`, `src/window/` | ✅ 正常使用 |
| `chrono` | 0.4 | `src/logger.rs` (时间戳生成) | ✅ 正常使用 |
| `rfd` | 0.15 | `src/services/wt.rs`, `src/settings/mod.rs` (原生对话框) | ✅ 正常使用 |
| `slint` / `slint-build` | 1 | `src/window/`, `src/settings/`, `build.rs`, `ui/*.slint` | ✅ 正常使用 |
| `raw-window-handle` | 0.6 | `src/platform/`, `src/window/`, `src/settings/` | ✅ 正常使用 |
| `tray-icon` | 0.24 (Win) | `src/platform/windows/mod.rs`, `src/tray/` | ✅ 正常使用 |
| `windows` | 0.58 (Win) | `src/platform/windows/mod.rs`, `src/services/` | ✅ 正常使用 |
| `embed-resource` | 3 (Build) | `build.rs` (Windows 图标/Manifest 嵌入) | ✅ 正常使用 |
| `tempfile` | 3 (Dev) | `tests/` 目录下多项单元/集成测试 | ✅ 正常使用 |

---

## 五、 Slint UI (ui/*.slint) 扫描

对 `ui/lib.slint`, `ui/lyric.slint`, `ui/settings.slint` 的所有元素、属性与回调进行检查：
1. `LyricWindow`: `lyric-text`, `font-size-px`, `font-family-name`, `text-color`, `outline-color`, `offset-x`, `pointer-pressed`, `pointer-released` 等属性与事件回调均在 `src/window/mod.rs` 中完整绑定。
2. `SettingsWindow`: 所有表单输入控件、按钮点击回调及下拉字体模型均与 `src/settings/mod.rs` 中的逻辑交互。
3. **结论**: UI 层无多余废弃控件或孤立属性。

---

## 六、 代码规范与 Clippy 告警统计 (Code Quality Warnings)

扫描过程中 Clippy 提出的代码改进项（非破坏性代码优化）：

1. **`src/config/mod.rs:216`**: `Config` 结构体手写了 `impl Default`，建议替换为自动 `#[derive(Default)]`。
2. **`src/settings/mod.rs:41` & `src/protocol/mod.rs:36`**: 手动切片 `&line[4..]`，建议使用 `strip_prefix("POS ")` 更安全地剥离前缀。
3. **`src/window/drag.rs:45`**: `process_pointer_input` 函数包含 8 个入参，建议封装为入参结构体。
4. **`src/window/mod.rs:86, 269`**: 不必要的 `.into()` 同类型转换 (`f32 -> f32`)。
5. **`src/window/mod.rs:93`**: `font_outline_width.max(0)` 为冗余判断。
6. **`src/window/mod.rs:334`**: 存在多层可合并的 `if` 嵌套条件。

---

## 七、 结论与清理建议

1. **死代码清理 (推荐进入阶段 3 重构计划)**:
   - 删除 `src/services/tray.rs` 文件及 `ServiceHandles` 中的 `tray` 字段。
   - 清理 `LyricUdpService::stop_listening`、`FontService` / `LyricUdpService` 中的死字段 `ctx`。
   - 删除 `ConfigService` 中未引用的 `save` / `reload_from_disk` 实例方法。
   - 移除 `PlatformWindowStyle::has_transparent_style` Trait 方法定义及其平台实现。
   - 移除 `RenderCache::playing_snapshot` 与 `WindowStyleService::is_locked`。
   - 清除 `AppError::SettingsUnavailable` 变体。

2. **静态资源清理**:
   - 安全移除或存档 `assets/icon.rc` 文件。

3. **依赖与 UI**:
   - Cargo 依赖项保持现有配置，无须调整 `Cargo.toml`。
