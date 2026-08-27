# Phase 2 Refactoring — PAL + Unified IPC Protocol — Scouting Findings

This file maps the scattered `#[cfg(windows)]` logic for tray, named pipes, window styles, mutex, and port check; and proposes the Platform Abstraction Layer (PAL), unified `src/protocol/mod.rs`, plus concrete file-level work.

## Files Retrieved

### Core entry points and modules
1. `src/lib.rs` (lines 1-34) — module registration; `#[cfg(windows)]` re-exports of `MutexHandle` (from `instance::mutex`), `probe_port` (from `instance::port`); provides `probe_port` stub for non-Windows.
2. `src/main.rs` (lines 1-145) — top-level wiring: CLI → mutex → config → logger → port bind → `AppContext` → pipe `reload` + `pos` servers → tray init → GUI run. Uses `instance::acquire_main_mutex` behind `#[cfg(windows)]`.
3. `src/pipe/mod.rs` (lines 1-12) — current cfg-based dispatcher: `windows_pipe` on Windows, `stub` elsewhere.
4. `src/pipe/windows_pipe.rs` (lines 1-322) — 322 lines containing:
   - `get_session_id()` (lines 28-37) — Win32 `ProcessIdToSessionId` call.
   - `get_pipe_name(base, session_id)` (lines 40-42) — returns `\\.\pipe\lyric-for-musicfox-{base}-{sid}`.
   - `connect_pipe()` (lines 47-86) — generic named-pipe client with retry.
   - `reload::start_server` / `notify_reload()` (lines 88-126) — reads "R", triggers `ctx.update_config`.
   - `pos::start_server` / `query_pos()` (lines 156-238) — exchanges `GET_POS\n` ↔ `x,y\n` or `EMPTY,EMPTY\n`.
   - `presence::start_server` / `try_activate_existing()` (lines 240-322) — `ACTIVATE {pid}\n` ↔ `OK\n`; uses `FindWindowW` + `SetForegroundWindow` + `AllowSetForegroundWindow`.
   - Text-protocol parsing is duplicated four times — the `IpcMessage` enum (per §3.2.2) would centralize this.
5. `src/pipe/stub.rs` (lines 1-29) — non-Windows noop; provides a different `get_pipe_name` format (no `\\.\pipe\` prefix).

### Tray
6. `src/tray/mod.rs` (lines 1-11) — cfg dispatcher (`windows_tray` vs `stub`).
7. `src/tray/windows_tray.rs` (lines 1-118) — uses `tray-icon` crate: builds `Menu`, `TrayIconBuilder`, two icons (`icon_normal`, `icon_gray`), spawns a thread polling `MenuEvent::receiver()` and `TrayIconEvent::receiver()`, emits `RequestRepaint` via `event_bus`, returns `TrayHandle { icon_normal, icon_gray, tray, flashing }`.
8. `src/tray/stub.rs` (lines 1-20) — duplicate `TrayCmd` enum + noop functions.

### Instance (mutex + port probe)
9. `src/instance/mod.rs` (lines 1-23) — module root with cfg dispatch + `pub use` re-exports.
10. `src/instance/mutex.rs` (lines 1-79) — `CreateMutexW` with named mutexes `Local\lyric-for-musicfox-main` and `Local\lyric-for-musicfox-settings`; on `ERROR_ALREADY_EXISTS` returns `AppError::AnotherInstance`.
11. `src/instance/port.rs` (lines 1-21) — `UdpSocket::bind` + immediate drop; `AddrInUse` → `AppError::PortInUse(port)`.
12. `src/instance/stub.rs` (lines 1-24) — non-Windows noop: `MutexHandle = ()`, always returns `Ok(None)`.

### Window (HWND / hit-test / monitor)
13. `src/window/hit_test.rs` (lines 1-141) — every function is `#[cfg(windows)]`-gated:
    - `hwnd_from_frame` (line 13) — `raw_window_handle` → `HWND`.
    - `apply_taskbar_and_locked_style` (line 43) — `GWL_EXSTYLE` toggles `WS_EX_TOOLWINDOW` / `WS_EX_APPWINDOW` / `WS_EX_TRANSPARENT`.
    - `apply_locked_style` (line 68) — alias for above.
    - `has_transparent_style` (line 79) — reads `WS_EX_TRANSPARENT`.
    - `apply_outer_position` (line 90) — `SetWindowPos` with `SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE`.
    - `refresh_stay_on_top` (line 109) — `SetWindowPos` with `HWND_TOPMOST`.
    - `window_outer_position` (line 120) — `GetWindowRect`.
    - `cursor_position` (line 132) — `GetCursorPos`.
14. `src/window/monitor.rs` (lines 1-180) — `#[cfg(windows)]` `enumerate()` uses `EnumDisplayMonitors` + `GetMonitorInfoW`; `primary_pixels_per_point()` uses `GetDpiForMonitor`; non-Windows falls back to 1920×1080.
15. `src/window/mod.rs` (lines 1-647) — `LyricApp` has 25+ public fields (lines 38-70) and 13 `#[cfg(windows)]` call sites: `apply_taskbar_and_locked_style`, `apply_outer_position`, `refresh_stay_on_top`, `hwnd_from_frame`, `cursor_position`, `window_outer_position`, `monitor_wnd_proc`, `SetWindowLongPtrW` hook, `PREV_WND_PROC`, `WND_CTX` thread-local, `monitor::enumerate`, `monitor::clamp_to_primary_monitor`, `monitor::primary_pixels_per_point`. P5 will fold these but P2 PAL must encapsulate them.

### Settings
16. `src/settings/mod.rs` (lines 1-300) — uses `pipe::presence::start_server` / `try_activate_existing` behind `#[cfg(windows)]` (lines 161, 176); defines `parse_pos_response()` (lines 28-44) that duplicates the protocol parse logic — should migrate to `IpcMessage::parse`. Has its own `windows_query_pos()` (lines 102-160) duplicating pipe-client connection logic.
17. `src/settings/form.rs` (line 158) and `src/settings/ui.rs` (line 104) — both call `crate::pipe::reload::notify_reload()`.
18. `src/settings/ui.rs` (line 547) — `#[cfg(windows)]` for `explorer.exe`; not in PAL scope but noted.

### Supporting
19. `src/event_bus/mod.rs` (lines 1-32) — `AppEvent::RequestRepaint / ConfigReloaded / TrayCmd / LyricStateChanged`; `bounded(256)`.
20. `src/context/mod.rs` (lines 1-55) — `AppContext` with `RwLock<Config>`, `RwLock<LyricState>`, `Signals { needs_reload, is_dragging, display_changed }`, `EventBus`.
21. `src/error.rs` (lines 1-65) — `AppError::AnotherInstance / MutexCreate / PortInUse / PosPipeTimeout`.

## Key Code — Scattered `#[cfg(windows)]` Hot Spots

```rust
// src/lib.rs:24-34
#[cfg(windows)] pub use instance::mutex::MutexHandle;
#[cfg(not(windows))] pub use instance::stub::MutexHandle;
#[cfg(windows)] pub use instance::port::probe_port;
#[cfg(not(windows))] pub fn probe_port(_port: u16) -> Result<(), AppError> { Ok(()) }
```

```rust
// src/main.rs:45-60 — mutex acquisition is cfg-gated
#[cfg(windows)] let _mutex_handle = match instance::acquire_main_mutex() { ... };
#[cfg(not(windows))] let _mutex_handle: instance::MutexHandle = ();
```

```rust
// src/window/hit_test.rs:1 — every function inside is cfg-gated
#[cfg(windows)] use windows::Win32::UI::WindowsAndMessaging::{ ... };
```

```rust
// src/pipe/windows_pipe.rs:28-42 — session ID + pipe-name logic
pub fn get_session_id() -> u32 { unsafe { /* Win32 */ } }
pub fn get_pipe_name(base: &str, session_id: u32) -> String {
    format!(r"\\.\pipe\lyric-for-musicfox-{}-{}", base, session_id)
}
```

```rust
// src/tray/windows_tray.rs:1-118 — uses tray-icon crate; non-Windows duplicate enum in stub.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCmd { OpenSettings, ReloadLyrics, Quit }  // ← duplicated in stub.rs
```

The protocol strings are scattered across 4 files (`windows_pipe.rs` × 3 modules + `settings/mod.rs::parse_pos_response`), confirming §3's "问题二（IPC 协议复制）" diagnosis.

## Architecture — Current State vs Target

```
Current (problem three — scattered cfg):
┌──────────────────────────────────────────────────────┐
│ main.rs / window/mod.rs / settings/ / tray/ / pipe/  │
│   ↑ each file has 1-13 #[cfg(windows)] attributes    │
│   ↑ protocol strings hand-coded in 4 places          │
└──────────────────────────────────────────────────────┘

Target (PAL + protocol):
┌──────────────────────────────────────────────────────┐
│ src/protocol/mod.rs       (IpcMessage, get_pipe_path)│  ← unit-testable on Linux
│ src/platform/trait.rs     (PAL trait contracts)      │
│ src/platform/windows/     (Win32 impls)              │
│ src/platform/stub/        (noop impls)               │
│ src/lib.rs → pub mod platform; pub mod protocol;     │
└──────────────────────────────────────────────────────┘
```

## Design — src/protocol/mod.rs

A single, OS-agnostic module that owns (a) the message enum and codec, (b) the pipe-name scheme, and (c) the session-ID concept.

```rust
// src/protocol/mod.rs — unit-testable on Linux; zero windows-API imports
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum IpcMessage {
    ReloadConfig,
    GetPosition,
    PositionResponse(Option<(i32, i32)>),
    ActivateSettings(u32),   // PID
    Success,
}

impl IpcMessage {
    pub fn to_bytes(&self) -> Vec<u8> { /* "RELOAD\n" / "GET_POS\n" / "POS x,y\n" /
                                            "POS EMPTY\n" / "ACTIVATE {pid}\n" / "OK\n" */ }
    pub fn parse(bytes: &[u8]) -> Result<Self, String> { /* symmetric */ }
}

/// Pipe-name scheme. `base` ∈ {"reload", "pos", "presence"}; session 0 on non-Windows.
pub fn get_pipe_path(base: &str, session_id: u32) -> String {
    // The doc §3.2.2 version prints \\.\pipe\lyric-for-musicfox-{base}-{sid};
    // on non-Windows platforms we currently use the format from src/pipe/stub.rs:8.
    // Final decision: keep a single string-formatting function; let platform
    // layer prepend \\.\pipe\ on Windows if needed.
    format!("lyric-for-musicfox-{}-{}", base, session_id)
}

/// Wraps the path with `\\.\pipe\` on Windows. Identity on other OSes.
pub fn platform_pipe_path(base: &str, session_id: u32) -> String {
    #[cfg(windows)]
    { format!(r"\\.\pipe\{}", get_pipe_path(base, session_id)) }
    #[cfg(not(windows))]
    { get_pipe_path(base, session_id) }
}

/// `get_session_id` lives here as a thin dispatcher that delegates to the
/// active PAL implementation (so the protocol module itself stays cfg-clean).
pub fn get_session_id() -> u32 {
    crate::platform::current().session_id()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn round_trip_reload() { /* … */ }
    #[test] fn parse_pos_empty() { /* … */ }
    #[test] fn parse_pos_with_negatives() { /* … */ }
    #[test] fn parse_pos_rejects_three_tokens() { /* … */ }
}
```

Migration moves:
- `src/pipe/windows_pipe.rs:28` `get_session_id` → `src/platform/windows/session.rs` (called by `protocol::get_session_id`).
- `src/pipe/windows_pipe.rs:40` `get_pipe_name` → `src/protocol/mod.rs::get_pipe_path` (and a wrapper that prepends `\\.\pipe\`).
- `src/pipe/windows_pipe.rs:188` `line.trim() == "GET_POS"` → `IpcMessage::parse`; reply becomes `IpcMessage::PositionResponse(opt).to_bytes()`.
- `src/pipe/windows_pipe.rs:267` `line.starts_with("ACTIVATE")` → `IpcMessage::parse`; reply `IpcMessage::Success.to_bytes()`.
- `src/pipe/windows_pipe.rs:312-322` `format!("ACTIVATE {}\n", …)` → `IpcMessage::ActivateSettings(pid).to_bytes()`; parse response with `IpcMessage::parse`.
- `src/pipe/windows_pipe.rs:108` `write_all(b"R")` (single byte reload signal) → consider migrating to `IpcMessage::ReloadConfig.to_bytes()` ("RELOAD\n"). Backward compat: keep emitting `b"R"` if external patching tools rely on the 1-byte format, OR emit "RELOAD\n" and update the server reader (single source of truth in PAL).
- `src/settings/mod.rs:28-44` `parse_pos_response` → re-export `IpcMessage::parse` for `PositionResponse`, or add a dedicated helper `fn parse_pos_message(bytes: &[u8]) -> Result<Option<(i32,i32)>, String>`.
- `src/settings/mod.rs:102-160` `windows_query_pos` → call `crate::platform::current().pipe_client().send_request(...)`.

## Design — src/platform/

```
src/platform/
├── mod.rs           ← `current() -> &'static dyn Platform` selector
├── trait.rs         ← Platform, PlatformTray, PlatformPipeServer, PlatformPipeClient,
│                       PlatformWindowStyle, PlatformInstance, PlatformSessionId
├── windows/
│   ├── mod.rs       ← impl Platform for WindowsBackend
│   ├── tray.rs      ← tray-icon (moved from src/tray/windows_tray.rs)
│   ├── pipe.rs      ← CreateNamedPipeW / CreateFileW (moved from src/pipe/windows_pipe.rs)
│   ├── window.rs    ← WS_EX_TRANSPARENT, refresh_stay_on_top (moved from src/window/hit_test.rs)
│   ├── instance.rs  ← CreateMutexW (moved from src/instance/mutex.rs)
│   ├── port.rs      ← UdpSocket::bind (moved from src/instance/port.rs)
│   └── session.rs   ← ProcessIdToSessionId
└── stub/
    ├── mod.rs       ← impl Platform for StubBackend
    ├── tray.rs      ← noop
    ├── pipe.rs      ← noop
    ├── window.rs    ← noop
    ├── instance.rs  ← always Ok(None)
    ├── port.rs      ← always Ok(())
    └── session.rs   ← returns 0
```

### Trait contracts (src/platform/trait.rs)

```rust
use std::sync::Arc;
use crate::context::AppContext;

pub trait PlatformSessionId {
    fn session_id(&self) -> u32;
}

pub trait PlatformInstance {
    fn acquire_main_mutex(&self) -> Result<Option<crate::platform::MutexHandle>, crate::error::AppError>;
    fn acquire_settings_mutex(&self) -> Result<Option<crate::platform::MutexHandle>, crate::error::AppError>;
    fn release_mutex(&self, h: Option<crate::platform::MutexHandle>);
    fn probe_port(&self, port: u16) -> Result<(), crate::error::AppError>;
}

pub trait PlatformTray {
    fn init(&self, ctx: Arc<AppContext>) -> Result<TrayHandles, String>;
    fn update_flash(&self, handles: &TrayHandles, flash: bool);
}

pub struct TrayHandles {
    pub inner: Option<crate::tray::TrayHandle>,   // Keep public type for backward compat
    pub cmd_rx: crossbeam_channel::Receiver<crate::tray::TrayCmd>,
}

pub trait PlatformPipeServer {
    fn start_reload(&self, ctx: Arc<AppContext>, config_path: std::path::PathBuf) -> Result<(), String>;
    fn start_pos(&self, ctx: Arc<AppContext>) -> Result<(), String>;
    fn start_presence(&self) -> Result<(), String>;
}

pub trait PlatformPipeClient {
    fn send(&self, base: &str, payload: &[u8], timeout_ms: u32) -> Result<Vec<u8>, String>;
    fn try_activate(&self, pid: u32) -> Result<(), String>;
}

pub trait PlatformWindowStyle {
    type Handle: Copy;
    fn from_frame(&self, frame: &eframe::Frame) -> Option<Self::Handle>;
    fn apply_locked_style(&self, h: Self::Handle, locked: bool);
    fn apply_outer_position(&self, h: Self::Handle, pos: (i32, i32));
    fn refresh_stay_on_top(&self, h: Self::Handle);
    fn window_outer_position(&self, h: Self::Handle) -> Option<(i32, i32)>;
    fn cursor_position(&self) -> Option<(i32, i32)>;
    fn has_transparent_style(&self, h: Self::Handle) -> bool;
    fn install_display_change_hook(&self, h: Self::Handle, ctx: Arc<AppContext>);
}

pub type MutexHandle = ();  // type alias — Windows re-defines via #[cfg]

pub trait Platform:
    PlatformSessionId + PlatformInstance +
    PlatformTray + PlatformPipeServer + PlatformPipeClient +
    PlatformWindowStyle
{}

pub fn current() -> &'static dyn Platform { /* cfg-selected static */ }
```

### Windows concrete type

```rust
// src/platform/windows/mod.rs
pub struct WindowsBackend;

impl Platform for WindowsBackend {}

impl PlatformSessionId for WindowsBackend { /* ProcessIdToSessionId */ }
impl PlatformInstance for WindowsBackend { /* CreateMutexW + UdpSocket::bind */ }
impl PlatformTray for WindowsBackend { /* tray-icon crate */ }
impl PlatformPipeServer for WindowsBackend { /* CreateNamedPipeW */ }
impl PlatformPipeClient for WindowsBackend { /* CreateFileW + WaitNamedPipeW */ }
impl PlatformWindowStyle for WindowsBackend {
    type Handle = windows::Win32::Foundation::HWND;
    /* … */
}
```

### Stub concrete type

```rust
// src/platform/stub/mod.rs
pub struct StubBackend;
impl Platform for StubBackend {}
impl PlatformSessionId for StubBackend { fn session_id(&self) -> u32 { 0 } }
impl PlatformInstance for StubBackend { /* always Ok(None); Ok(()) */ }
impl PlatformTray for StubBackend { /* noop TrayHandles { inner: None, cmd_rx: unbounded } */ }
impl PlatformPipeServer for StubBackend { /* Ok(()) */ }
impl PlatformPipeClient for StubBackend { /* Err("not supported") */ }
impl PlatformWindowStyle for StubBackend { type Handle = (); /* all fns noop */ }
```

`src/platform/mod.rs` selects via `#[cfg(windows)] static WINDOWS: WindowsBackend = …; #[cfg(not(windows))] static STUB: StubBackend = …;` and `pub fn current() -> &'static dyn Platform { #[cfg(windows)] { &WINDOWS } #[cfg(not(windows))] { &STUB } }`.

## Files to Modify or Create

### Create (new)
1. `src/protocol/mod.rs` — `IpcMessage`, `get_pipe_path`, `get_session_id`, codec tests.
2. `src/platform/mod.rs` — `current()` selector, re-exports.
3. `src/platform/trait.rs` — All PAL trait definitions.
5. `src/platform/windows/mod.rs` + `tray.rs`, `pipe.rs`, `window.rs`, `instance.rs`, `port.rs`, `session.rs` — Windows concrete impls.
6. `src/platform/stub/mod.rs` + `tray.rs`, `pipe.rs`, `window.rs`, `instance.rs`, `port.rs`, `session.rs` — Stub noop impls.

### Modify
1. `src/lib.rs` — register `pub mod platform; pub mod protocol;`; replace `#[cfg(windows)] pub use …` re-exports with `pub use platform::*`.
2. `src/main.rs:45-60` — drop `#[cfg(windows)]` on mutex acquire; use `platform::current().acquire_main_mutex()`.
3. `src/main.rs:124-125` — replace `lyric_for_musicfox::pipe::reload::start_server(...)` and `pipe::pos::start_server(...)` with `platform::current().start_reload/start_pos`.
4. `src/tray/mod.rs` — keep the public `TrayCmd` enum and `TrayHandle` (re-exported from `platform::tray` or kept in-place). `init_tray` / `update_tray_flash` become thin shims that call `platform::current().init/update_flash`.
5. `src/pipe/mod.rs` + `src/pipe/{windows_pipe,stub}.rs` — remove; redirect top-level `pub use platform::pipe::{start_reload, start_pos, start_presence, send_request, try_activate}` to keep callers compiling.
6. `src/instance/mod.rs` + `src/instance/{mutex,port,stub}.rs` — remove; `lib.rs::instance` becomes a thin re-export from `platform::instance`.
7. `src/window/hit_test.rs` — body moves to `src/platform/windows/window.rs` (and `src/platform/stub/window.rs`). Public surface kept as `pub use crate::platform::window::*` for incremental migration.
8. `src/window/monitor.rs` — `enumerate` and `primary_pixels_per_point` stay (they are already cfg-clean thanks to internal cfg dispatch); no change required for P2, though they could be lifted into `platform::monitor` in P3.
9. `src/window/mod.rs` — change call sites at lines 96, 121, 138-152, 186-203, 369-388, 405-410, 427, 549, 562 to call `platform::current().apply_locked_style(...)` etc.; drop `PREV_WND_PROC` static (windows-only hook) into `platform::windows::window::install_display_change_hook`.
10. `src/settings/mod.rs:28-44` — replace `parse_pos_response` with `IpcMessage::parse`-based wrapper.
11. `src/settings/mod.rs:78-93` — drop `current_session_id`; call `crate::platform::current().session_id()`.
12. `src/settings/mod.rs:102-160` — `windows_query_pos` body moves into `platform::windows::pipe::send_request`.
13. `src/settings/mod.rs:161, 176` — call `platform::current().start_presence()` / `try_activate`.
14. `src/settings/form.rs:158` and `src/settings/ui.rs:104` — keep `crate::pipe::reload::notify_reload()` as a top-level shim that calls `platform::current().send(...)`.

## Constraints and Risks

1. **External patching tool compatibility** (per `docs/dev-stage-2/refactor_implementation.md` §7 "一致性检验"): `musicfox-patch` and external tools may depend on the existing 1-byte reload signal `b"R"` and the legacy `EMPTY,EMPTY` pos format. `IpcMessage::ReloadConfig.to_bytes()` emits `"RELOAD\n"` (5 bytes). Two options:
   - Strict migration: emit "RELOAD\n", update patch tools.
   - Backward compatible: keep emitting `b"R"` for reload, and emit `"POS EMPTY\n"` instead of `"EMPTY,EMPTY\n"`. The protocol enum can carry both wire formats via a `to_wire_legacy()` companion.
   Recommend documenting the choice in the protocol module doc-comment.
2. **Tray enum duplication** (`tray::windows_tray::TrayCmd` vs `tray::stub::TrayCmd`): single source of truth in `src/tray.rs` (or keep `src/tray/mod.rs` only declaring `pub enum TrayCmd { … }` and have the platform layer reference it). Risk: stub currently lives in a different file with its own copy.
3. **HWND handle as `*mut c_void`**: PAL `PlatformWindowStyle::Handle` is associated type `type Handle: Copy;` — Windows uses `HWND` (raw pointer), stub uses `()`. P5 LyricApp field `pub hwnd` must change to `Option<<dyn PlatformWindowStyle>::Handle>` (or be wrapped behind a service). For P2 the simplest path: keep `LyricApp::hwnd` as `Option<HWND>` for now but introduce a service handle in P3.
4. **`monitor_wnd_proc` global static** (`src/window/mod.rs:18` `static mut PREV_WND_PROC`): unsafe mutable static. Move into `platform::windows::window::DisplayChangeHook` struct with internal `OnceLock<Mutex<…>>` to comply with Rust 2024 `static_mut` warnings and P5's "no globals" goal.
5. **Thread-local `WND_CTX`** (`src/window/mod.rs:22-26`): thread_local with `RefCell<Option<Arc<AppContext>>>` — moves to platform module, but threading the `Arc<AppContext>` into the WndProc must be done at hook-install time, not every call.
6. **`raw-window-handle` 0.6** is already declared in `Cargo.toml`; `windows = "0.58"` features already include `Win32_System_RemoteDesktop` and `Win32_System_Pipes` — feature set is complete, no Cargo.toml changes anticipated for P2.
7. **Stub `get_pipe_name` format differs from Windows** (`src/pipe/stub.rs:8` returns `"lyric-for-musicfox-…"` not `"\\.\pipe\lyric-for-musicfox-…"`). If protocol module normalises to Windows format on all platforms, stub code path on Linux becomes a noop anyway — no breakage, but the test should not assert the legacy stub format. Document this in `protocol/mod.rs`.
8. **`tray_handle.icon_normal/icon_gray` Rc fields** (`src/tray/windows_tray.rs:21-22`): the existing handle exposes two `Rc<tray_icon::Icon>` references. PAL `PlatformTray::init` returns `TrayHandles` whose `inner` is `Option<TrayHandle>` — same shape — so the public field path stays valid through P2 and P4 can drop the unused `icon_gray` field.
9. **`Arc<AppContext>` ownership across threads**: pipe servers already take `Arc<AppContext>` and call `ctx.update_config()`; the protocol refactor doesn't change this, but `protocol/mod.rs::get_session_id` must not depend on `AppContext` to remain cfg-pure.

## Open Questions (recommend Worker confirms)

1. Should `IpcMessage::ReloadConfig` wire-format migrate to `"RELOAD\n"` (breaking patch tools) or keep `b"R"` (single byte)?
2. Should `PlatformWindowStyle::install_display_change_hook` be a method (called once after HWND is acquired) or stay as a free function in `platform::windows::window`?
3. `PlatformPipeServer::start_*` returns `Result<(), String>` — should they spawn-and-forget (current behavior) or expose a `JoinHandle` for P3 service-lifecycle management?
4. P2 vs P3 split: §4 mentions `LyricUdpService`; `src/lyric/udp.rs` already exists and is not cfg-gated. P3 will own it. P2 should not touch UDP.

## Start Here

Open `src/pipe/windows_pipe.rs` lines 28-130 (`get_session_id`, `get_pipe_name`, `reload::start_server`). It is the single file with the highest concentration of (a) platform-specific FFI, (b) duplicated protocol string literals, and (c) duplicated connection-loop boilerplate. Establishing `src/protocol/mod.rs::IpcMessage` first makes the other refactors mechanical.

## Supervisor Coordination

No decision is blocked. The two clarifications above (item 1 reload wire-format, item 2 hook API shape) can be defaulted by the Worker using the analysis-section defaults (legacy-compatible reload byte, free-function hook) without interrupting the scout workflow.