# Dependabot Security Alert #1 — Fix Context

## Bug

GHSA-wrw7-89jp-8q8g / RUSTSEC-2024-0429：
`glib` 的 `VariantStrIter::impl_get` 接受 `&NULL` 并通过 variadic `g_variant_get_child` 调用，产生 UB / NULL deref。
受影响的版本范围：`>= 0.15.0, < 0.20.0`；首个修复版：`0.20.0`。

仓库 Dependabot alert #1 报告：`glib 0.18.5` 在 lockfile 中（命中 vulnerable 区间）。

## 根因

`Cargo.toml` 的 Windows 依赖 `tray-icon = "0.19"` 通过
`tray-icon → libappindicator 0.9.0 → glib ^0.18, gtk ^0.18` 的强依赖链
拉入 `glib 0.18.5`。

直接调用 `tray_icon::Icon / TrayIconBuilder / TrayIconEvent / MouseButton*` 等符号
全部位于 Windows 路径（`src/platform/windows/mod.rs`、`src/tray/mod.rs`、`build.rs`），
Linux/Mac 的 `libappindicator` / `gtk` 运行时支持对本项目不可达，但作为传递依赖仍参与
`cargo` 依赖解析与 lockfile 锁定，因此触发了 Dependabot 告警。

## 修复

`tray-icon 0.24.0`（changelog 2026-05-07）把 `libappindicator` 改为 **optional**，
并把整个 `gtk` feature 设为 `default = ["libxdo", "gtk"]`。
采用 `default-features = false` + 仅启用 `libxdo`，即可在所有平台上完全避开
`libappindicator` / `gtk` / `glib` 依赖传递。

`tray-icon 0.24.x` 没有破坏 `Icon::from_resource / from_rgba`、`TrayIconBuilder`、
`TrayIconEvent::{Click, DoubleClick}`、`MouseButton`、`MouseButtonState` 的 API，
本仓库的 Windows 调用点全部为这些稳定接口，因此可直接升级。

## 修改文件

- `Cargo.toml` line 48
  - before: `tray-icon = "0.19"`
  - after:  `tray-icon = { version = "0.24", default-features = false, features = ["libxdo"] }`
  - 附带说明此约束源于 GHSA-wrw7-89jp-8q8g。
- `Cargo.lock`
  - `tray-icon 0.19.3 → 0.24.2`
  - 整条 `libappindicator*`、`gtk*`、`cairo-rs*`、`pango*`、`gdk*`、`atk*`、`gio*`、`gobject-sys*`、`glib*`、`gdk-pixbuf*`、`muda 0.15` 等 90+ 个传递依赖被删除（净 −491 行）。

## 验证

| 命令 | 结果 |
|---|---|
| `grep -cE '^name = "(glib\|gtk\|libappindicator\|cairo-rs\|pango\|gdk)"' Cargo.lock` | `0`（修复前 ≥ 9） |
| `grep -A1 '^name = "tray-icon"' Cargo.lock` | `0.24.2` |
| `cargo check --target x86_64-pc-windows-gnu` | ✅ Finished in 29.62s |
| `cargo check --target x86_64-pc-windows-gnu --all-targets` | ✅ Finished in 26.10s |
| `cargo test --target x86_64-pc-windows-gnu --lib --bins` | ✅ 11 passed; 0 failed |

未在 MSVC target 验证（rustup 未安装 `x86_64-pc-windows-msvc` std 库），与本次修复无关。

## 剩余风险

- `cargo update` 后续仍可能拉入 `tray-icon 0.24` 系列小版本（如 0.24.3+），这些版本继续保留
  `libappindicator` 作为 optional。若上游某次小版本错误地把 `gtk` 重新设为默认 feature，
  会再次引入 `glib 0.18`。建议在 CI 中追加 `cargo tree -i glib -i libappindicator` 检查，
  或升级时关注 tray-icon CHANGELOG。
- Linux 编译时若启用 `default-features`，会拉回 `glib 0.18`。当前 Cargo.toml 已关闭默认 feature。
- Mac 编译路径不受影响（`tray-icon 0.24` 的 macOS deps 与 `glib` 无关）。

## 下一步（可选）

1. 在仓库里新增 `.github/dependabot.yml`（目前缺失），启用 cargo-ecosystem 自动监控。
2. 在 CI 中加 `cargo audit`（或 `cargo deny`）作为补充。
3. 通过 `gh api repos/xxbern/lyric-for-musicfox/dependabot/alerts/1 -X PATCH` 把 alert 标记为 fixed。
