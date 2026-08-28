# Code Context

## Files Retrieved
1. `src/services/wt.rs` (lines 31-118) — Implements `WtService::toggle` where WT window is found or launched; contains the path resolution logic and WT process launching, but lacks pre-launch validation and error prompt/fallback to settings.
2. `src/window/mod.rs` (lines 339-385) — Handles `TrayCmd::ToggleWt` and `TrayCmd::OpenSettings` in `handle_tray_cmd`. Demonstrates standard mechanism for spawning settings (`std::process::Command::new(exe).arg("--settings").spawn()`).
3. `src/settings/validate.rs` (lines 65-83, 100-128) — Implements `validate_musicfox_path` which validates that `musicfox_path` is non-empty and points to an existing file on disk.
4. `src/settings/mod.rs` (lines 227-258, 375-392) — Settings entry point and GUI lifecycle. Implements single-instance presence activation via named pipe (`presence-{session_id}`) and uses `rfd::FileDialog` for file selection.
5. `src/platform/windows/mod.rs` (lines 173-207, 1044-1055) — Tray event listener dispatching `TrayCmd::ToggleWt` on left click, and `launch_wt` spawning `wt.exe`.
6. `src/platform/trait.rs` (lines 55-66) — `PlatformWt` trait definition specifying `find_wt_window`, `launch_wt`, and window styling APIs.
7. `src/config/mod.rs` (lines 229-247) — `WtConfig` struct schema with `musicfox_path`, `app_dir`, and `title`.
8. `tests/validate_test.rs` (lines 62-72) — Unit tests for `validate_musicfox_path`.

## Key Code

### 1. `WtService::toggle` in `src/services/wt.rs`
```rust
pub fn toggle(&self, config: &WtConfig) {
    // 1. 全局 400ms 防抖
    // 2. 检查是否正在启动过程中
    // 3. 动态读取磁盘最新配置
    let current_cfg = crate::services::config::ConfigService::load_or_default()
        .unwrap_or_else(|_| self.ctx.get_config());
    let effective_config = if !current_cfg.wt.musicfox_path.is_empty() {
        &current_cfg.wt
    } else {
        config
    };

    if let Some(hwnd) = crate::platform::current().find_wt_window(&effective_config.title) {
        if crate::platform::current().is_wt_hidden(hwnd) {
            crate::platform::current().set_wt_alpha(hwnd, 255);
            crate::platform::current().activate_wt(hwnd);
        } else {
            crate::platform::current().set_wt_alpha(hwnd, 0);
        }
    } else {
        // [TARGET CHANGE AREA]
        // If musicfox_path is empty or file does not exist:
        // 1. Show prompt dialog (via rfd::MessageDialog)
        // 2. Launch settings interface (--settings)
        // 3. Return early
        ...
    }
}
```

### 2. Path Validation in `src/settings/validate.rs`
```rust
pub fn validate_musicfox_path(path_str: &str) -> Result<(), String> {
    let s = path_str.trim();
    if s.is_empty() {
        return Err("go-musicfox 路径不能为空".into());
    }
    #[cfg(windows)]
    {
        let p = std::path::Path::new(s);
        if !p.exists() {
            return Err("go-musicfox 可执行文件不存在".into());
        }
    }
    #[cfg(not(windows))]
    {
        let p = std::path::Path::new(s);
        if !s.starts_with("C:") && !s.starts_with("c:") && !p.exists() {
            return Err("go-musicfox 可执行文件不存在".into());
        }
    }
    Ok(())
}
```

### 3. Launching Settings & Dialog Prompting
- **Settings launch**:
```rust
if let Ok(exe) = std::env::current_exe() {
    let _ = std::process::Command::new(exe).arg("--settings").spawn();
}
```
- **Dialog Prompting (`rfd::MessageDialog`)**:
`rfd = "0.15"` is already in `Cargo.toml`. `rfd::MessageDialog::new().set_title(...).set_description(...).set_level(rfd::MessageLevel::Warning).set_buttons(rfd::MessageButtons::Ok).show()` is synchronous. Spawning the dialog on a dedicated thread prevents blocking the Slint GUI ticker.

## Architecture
- **Tray Event Flow**: When the user single left-clicks the system tray icon, `tray-icon` fires a click event captured in `src/platform/windows/mod.rs`, which sends `TrayCmd::ToggleWt` over a crossbeam channel.
- **Window Event Loop**: In `src/window/mod.rs`, `tick()` receives `TrayCmd::ToggleWt` and delegates to `services.wt.toggle(&ctx.get_config().wt)`.
- **WT Toggle & Launch**:
  - `WtService::toggle` re-reads current configuration from disk.
  - If a terminal window with `title` is found, it toggles between hidden (alpha 0) and shown (alpha 255 + activated).
  - If no window is found, it checks `validate_musicfox_path(&effective_config.musicfox_path)`.
  - When invalid/missing:
    - Displays a user prompt (via `rfd::MessageDialog` or native message box in a thread).
    - Spawns the settings process (`--settings`), which either opens the settings window or activates the existing settings window via presence pipe.
    - Avoids setting `IS_LAUNCHING = true` or calling `launch_wt`.
  - When valid: launches `wt.exe`, hooks the window after 1s, and marks launching complete.

## Start Here
Start at `src/services/wt.rs` (lines 50-80) inside `WtService::toggle`:
Add the path validity check before the `find_wt_window` else branch / WT launch, spawn the warning dialog, and launch `--settings`.
