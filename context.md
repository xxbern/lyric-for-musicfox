# Multi-Monitor DPI Scaling Drag Bug — Scouting Findings

Project: `lyric-for-musicfox` (Windows-only Slint-based desktop lyrics overlay; Rust 2021 edition; WSL dev → Windows runtime).
Bug severity: **High** — lyric window becomes undraggable while straddling 2.5K (150% DPI) and 1080P (100% DPI) monitors; window size pumps/grows on each tick.

This file is **scout-only**: no source files were modified.

---

## Files Retrieved (with rationale)

1. `src/window/mod.rs` (lines 1-454) — top-level lyric window lifecycle; `run()`, `tick()`, `reload_config()`. Hosts the `monitor_wnd_proc` hook + drag loop + display-change poll.
2. `src/window/drag.rs` (lines 1-110) — pure-function drag state machine. `process_pointer_input` returns new `(x, y)` based on `(anchor_window, anchor_cursor, current_cursor)`.
3. `src/platform/windows/mod.rs` (lines 547-595 and 600-647, plus 857-872) — `monitor_wnd_proc` (lyric WM_DPICHANGED handler), `settings_dpi_wnd_proc` (settings handler), and `install_display_change_hook` that subclasss the lyric window WndProc.
4. `src/platform/trait.rs` (lines 1-65) — PAL trait surface (`PlatformWindowStyle::install_display_change_hook`).
5. `src/services/monitor.rs` (lines 1-150) — virtual-desktop pixel coords, `clamp_to_primary_monitor`, `primary_pixels_per_point`. Note `clamp_to_primary_monitor` is **not DPI-aware** (uses primary monitor's `rcMonitor`).
6. `src/services/position.rs` (lines 1-40) — `PositionService::apply_window_pos` = `apply_outer_position + set_current_pos`.
7. `src/context/mod.rs` (lines 1-50) — `Signals { is_dragging, display_changed }`. Used to gate the 5 s clamp-poll inside `tick` step 4.
8. `src/lyric/state.rs` (lines 1-50) — `LyricState { pos_x, pos_y, … }`. Persisted in memory only; not DPI-tagged.
9. `src/config/mod.rs` (lines 10-220) — `WindowConfig { width, height, pos_x, pos_y, … }`. **width/height are DIP (logical, see `default_width = 800`)**, **pos_x/pos_y are virtual-desktop physical pixels** (req.md §7.2).
10. `ui/lyric.slint` (lines 1-150) — `LyricWindow` is `no-frame`, transparent, always-on-top. Only the inner `Text` element (wrapped in `TouchArea`) is draggable — **the entire drag-region = text element bounds**.
11. `tests/drag_test.rs` (lines 1-100) — drag unit tests; cover anchor, release, lock. **No DPI / multi-monitor coverage.**
12. `tests/monitor_test.rs` (lines 1-150) — `clamp_to_primary_monitor` unit tests; assumes same-DPI coordinates.
13. `docs/context.md` (lines 1-360) — prior P2 scout notes about PAL refactor; confirms `monitor_wnd_proc` subclasses the Slint WndProc and is the only DPI hook.
14. `docs/dev-stage-3/review-finding.md` §8 (lines 256-275) — explicitly flags `install_display_change_hook` permanently overriding Slint's WndProc; recommends `window.on_scale_factor_changed` instead.
15. `docs/dev-stage-1/req.md` §7.2 / §11 — **size fields = DIP; position fields = virtual-desktop physical pixels; cross-DPI drag must not remap coordinates**.

## Key Code

```rust
// src/window/mod.rs:55-70 (initial position resolution)
let win_size = (
    (config.window.width as f32 * ppp).round() as i32,   // 800 DIP * 1.5 = 1200 phys
    (config.window.height as f32 * ppp).round() as i32,
);

// src/window/mod.rs:118-124 (initial size set — **no DPI multiplier!**)
app.window().set_size(slint::PhysicalSize::new(
    config.window.width.max(1),     // 800 → passed as PHYSICAL pixels ← wrong unit
    config.window.height.max(1),
));

// src/window/mod.rs:269-298 (5 s clamp poll, gated by !is_dragging)
let ppp = services.monitor.primary_pixels_per_point(&layout);   // primary only
let size_phys = (
    (r.config.window.width as f32 * ppp).round() as i32,
    (r.config.window.height as f32 * ppp).round() as i32,
);
let clamped = services.monitor.clamp_position(current_pos, &layout, size_phys);
```

```rust
// src/window/mod.rs:307-329 (per-tick drag path)
let new_pos = drag::process_pointer_input(  // returns Some(new_outer_position)
    &mut r.drag, &mut guard, &ctx.signals, locked,
    current_physical, left_pressed, left_released, current_outer,
);
if let (Some(pos), Some(hwnd)) = (new_pos, r.hwnd) {
    services.position.apply_window_pos(hwnd, pos);   // → SetWindowPos(SWP_NOSIZE)
    services.position.set_current_pos(pos);
}
```

```rust
// src/window/drag.rs:30-105 (drag math)
pub fn compute_new_position(anchor_physical, anchor_window, current_physical) {
    let dx = current_physical.0 - anchor_physical.0;
    let dy = current_physical.1 - anchor_physical.1;
    (anchor_window.0 + dx, anchor_window.1 + dy)
}
// anchor_window is captured once on press and NEVER updated again until release.
```

```rust
// src/platform/windows/mod.rs:548-595 — LYRIC WM_DPICHANGED handler
pub unsafe extern "system" fn monitor_wnd_proc(hwnd, msg, wparam, lparam) -> LRESULT {
    if msg == 0x007E { /* WM_DISPLAYCHANGE → signals.display_changed = true */ }
    else if msg == 0x02E0 {  // WM_DPICHANGED
        let is_dragging = (GetKeyState(0x01 /*VK_LBUTTON*/) as i16) < 0;
        if is_dragging {
            let hmon = MonitorFromPoint(cursor_pt, MONITOR_DEFAULTTONEAREST);
            let cursor_dpi_x = GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI);
            let msg_dpi_x = (wparam.0 & 0xFFFF) as u32;
            if cursor_dpi_x != msg_dpi_x { return LRESULT(0); }   // GUARD
        }
        // Falls through → resize + reposition to suggested rect
        let prc = lparam.0 as *const RECT;
        let r = *prc;
        SetWindowPos(hwnd, HWND_TOP, r.left, r.top,
                     r.right - r.left, r.bottom - r.top,
                     SWP_NOZORDER | SWP_NOACTIVATE);   // ← no SWP_NOMOVE/SWP_NOSIZE
        return LRESULT(0);
    }
    if let Some(prev) = get_prev_wnd_proc() { CallWindowProcW(prev, hwnd, msg, wparam, lparam) }
    else { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
```

```rust
// src/platform/windows/mod.rs:600-647 — same pattern in settings_dpi_wnd_proc
// (settings window also has identical DPI guard).
```

```rust
// src/platform/windows/mod.rs:857-872
fn install_display_change_hook(&self, hwnd, ctx) {
    // Replaces Slint's WndProc with monitor_wnd_proc (one-shot, no teardown).
    SetWindowLongPtrW(hwnd, GWL_WNDPROC, monitor_wnd_proc as *const () as isize);
    // Slint's original WndProc is stored in PREV_WND_PROC and called via CallWindowProcW.
}
```

```rust
// src/platform/windows/mod.rs:809-820 — apply_outer_position (drag path)
SetWindowPos(hwnd, HWND_TOP, pos.0, pos.1, 0, 0,
             SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
```

```slint
// ui/lyric.slint:1-150 — window has no_frame, transparent; only txt (Text) has TouchArea
// The TouchArea covers ONLY the text bbox, not the whole window area.
```

## Architecture (relevant subset)

```
Slint event loop (16 ms tick)
  ├─ on_pointer_pressed / on_pointer_released (Slint callbacks → Rc<Cell<u32>>)
  └─ tick() in window/mod.rs
       1. Drain event_bus / tray_cmd
       2. Snap lyric state; apply style + scroll
       3. (skipped if is_dragging) 5 s display-change poll → clamp_to_primary_monitor + apply_window_pos
       4. drag::process_pointer_input → SetWindowPos(SWP_NOSIZE)  ← per-tick drag
       5. 500 ms stay-on-top refresh

WndProc chain (overridden once at hwnd acquisition)
  monitor_wnd_proc
    ├─ WM_DISPLAYCHANGE → signals.display_changed = true
    ├─ WM_DPICHANGED   → DPI guard (cursor vs wparam), else SetWindowPos to suggested rect
    └─ default → CallWindowProcW(prev, ...) → Slint's WndProc

  Note: monitor_wnd_proc swallows WM_DPICHANGED (return LRESULT(0) without forwarding).
        Slint therefore NEVER sees WM_DPICHANGED; its scale_factor() is whatever was
        cached at startup (1.0 on most workstations where the primary monitor is 100%).
```

## Observed vs Expected Behavior

### Observed (user report)

1. Drag lyric window from 2.5K (150 % DPI) toward 1080P (100 % DPI) monitor.
2. **As soon as even a few pixels of the window's bottom edge remain on the 2.5K monitor**, the window:
   - stops responding to drag (cursor leaves the Text/TouchArea region), AND
   - appears to grow on each subsequent tick (physically larger, often off-screen on the 1080P side).
3. Release mouse does not fully recover; sometimes user must kill the process.

### Expected

1. Window should snap to the target monitor's DPI **once** as it crosses the boundary.
2. While dragging across DPI boundaries, the window must stay draggable: cursor→window offset must remain constant in physical pixels.
3. No oscillation / pumping / unbounded growth of physical size during drag.

## Root Cause (with evidence)

**There are three contributing defects; the dominant one is defect #1.**

### Defect #1 — WM_DPICHANGED handler bypasses guard when cursor monitor == suggested DPI (the 1080P + 2.5K straddle case)

`src/platform/windows/mod.rs:559-587` (`monitor_wnd_proc` WM_DPICHANGED branch):

- The guard compares **`cursor_dpi_x`** (DPI of the monitor containing the cursor) with **`msg_dpi_x`** (DPI Windows suggests for the window, which is the DPI of the cursor's monitor per MSDN).
- **In the user's straddle scenario**: cursor sits on 1080P → `cursor_dpi_x = 96`. Windows sends `WM_DPICHANGED(wparam=96, …)` because the cursor is on the 1080P monitor → `msg_dpi_x = 96`. **They match** → guard is bypassed.
- The handler then calls `SetWindowPos(hwnd, HWND_TOP, r.left, r.top, r.right-r.left, r.bottom-r.top, SWP_NOZORDER | SWP_NOACTIVATE)`. The flags intentionally **omit `SWP_NOMOVE`** and **`SWP_NOSIZE`** (per MSDN: WM_DPICHANGED handler should adopt the suggested rect fully).

Consequence: every WM_DPICHANGED during a drag (fired whenever the cursor crosses an internal DPI boundary, even by one pixel) **re-positions AND re-sizes the window** to the suggested rect. The user's drag is overwritten; the next 16 ms tick's `drag::process_pointer_input` then fights back with `SetWindowPos(SWP_NOSIZE)`.

**Why the window "grows"** instead of just pumping: at 150 % → 100 % DPI, Windows' suggested rect is *smaller* (800 × 53 vs 1200 × 80 phys). But the drag system's anchor is `(old_position, old_size)`. The cursor is still moving toward the 1080P centre, and the drag-expected position overshoots. As the cursor sweeps back over the boundary (or stays near the seam), every few ticks the cursor monitor flips and WM_DPICHANGED fires again. The handler keeps adopting the larger suggested rect (e.g. when cursor is on 2.5K → 1200 × 80) **and simultaneously repositions the window to the cursor-aligned location**. The drag system then pulls the window back to its anchor-derived position with `SWP_NOSIZE` (size preserved as large), so the window effectively grows and gets pinned near the boundary, repeatedly.

**Why undraggable**: `ui/lyric.slint:140-150` — only the inner `Text` + `TouchArea` accepts pointer events. The window's hit-testable area = the rendered text bbox. After the WM_DPICHANGED resize, `Slint` never received the WM_DPICHANGED (we return `LRESULT(0)` without forwarding to `prev`), so its cached `scale_factor()` is stale (still 1.0 from primary monitor). The text is laid out in stale-DIP space; the `TouchArea` no longer covers the actual visible window region. Cursor leaves the TouchArea → `pointer_event(down/up)` stops firing → `process_pointer_input` never sees `left_pressed`/`left_released` again. Drag is effectively frozen while `signals.is_dragging == true` (kept true until the next `pointer-released`, which never arrives).

### Defect #2 — `monitor_wnd_proc` swallows WM_DPICHANGED without forwarding to Slint

`src/platform/windows/mod.rs:548-595` — every WM_DPICHANGED returns `LRESULT(0)` directly (or after the resize block). This means:

- `Slint::Window::scale_factor()` **never updates** (it's only re-read in `tick` step 4 and `reload_config`).
- Slint's internal render tree still assumes the original scale factor, so logical-to-physical mapping goes stale.
- `app.window().scale_factor()` returns 1.0 even though the window is now drawn at 100 % DPI while the user thinks it is at 150 %. (Symptom: text may render with wrong DPI metrics, font may look wrong after a cross-DPI drag.)

### Defect #3 — `apply_outer_position`/`apply_window_pos` use `SWP_NOSIZE`, freezing the size after WM_DPICHANGED's resize

`src/platform/windows/mod.rs:809-820` — the drag tick uses `SWP_NOSIZE`, so once WM_DPICHANGED has set a new size, the drag system keeps that new size forever (it does not recompute size based on DPI). Combined with defect #1, this is what makes the size change "stick" rather than oscillate.

### Secondary issues (not the root cause, but worsen it)

- `src/window/mod.rs:118-123` (initial size) passes `cfg.window.width` (logical 800) directly to `slint::PhysicalSize::new(...)` — **missing the `* ppp` multiplier**. Only `reload_config` (line 368) multiplies by `scale_factor()`. After a config reload the window snaps to the correct physical size, but on first launch and after WM_DPICHANGED the size is wrong.
- `src/services/monitor.rs:124-145` `clamp_to_primary_monitor` uses **primary monitor DPI** for `size_phys` even when the window is on a different monitor — would mis-clamp during the 5 s poll.
- `src/window/mod.rs:269-298` step 4 is gated by `!is_dragging`, so the 5 s clamp cannot rescue the user mid-drag.
- `src/platform/windows/mod.rs:857-872` `install_display_change_hook` permanently overrides Slint's WndProc with no teardown (review-finding.md §8 already flagged this).

## Tests (existing & proposed)

### Existing coverage

| File | Covers | Gap |
|------|--------|-----|
| `tests/drag_test.rs` | drag math, anchor, lock, release | No DPI / straddle cases |
| `tests/monitor_test.rs` | `clamp_to_primary_monitor` single-monitor | No multi-DPI / off-primary monitor |
| `tests/scroll_test.rs` | scroll state | Unrelated |
| `tests/refactor_p1_test.rs` | config round-trip | Unrelated |

### Validation strategy (manual — required for any fix)

1. **Dual-monitor with different DPI**: 1080P at 100 % + 2.5K at 150 %, side-by-side.
   - Drag the lyric window from 2.5K onto 1080P, slowing down so the bottom edge stays on 2.5K for ≥ 1 s.
   - Expected: window scales to 100 % DPI **once**; cursor remains inside the draggable text area; window continues to follow the cursor with constant physical offset.
2. **Drag fully onto 1080P**: window should stay at 100 % DPI until dragged back.
3. **Drag fully onto 2.5K**: window should grow back to 150 % DPI exactly once.
4. **Drag back-and-forth across the seam** (10 oscillations in 5 s): window must not grow beyond the 150 % size; cursor must remain over the draggable area at all times.
5. **Lock toggle during straddle**: should not crash; window should remain at the cursor's monitor DPI.
6. **Unit tests** to add (recommended):
   - `drag::compute_new_position` is already pure; no change needed.
   - Add `tests/dpi_guard_test.rs` (Windows-only) that constructs a fake HWND context and exercises `monitor_wnd_proc`'s DPI guard logic via a helper that takes `cursor_dpi, msg_dpi, is_dragging` and returns `Option<RECT>`. Hard to do without Win32, so prefer a thin `fn decide_dpi_action(cursor_dpi, msg_dpi, is_dragging) -> DpiAction` extracted from the handler and unit-tested on Linux.
   - Add a regression `drag_test::straddle_size_does_not_grow`: simulate two WM_DPICHANGED events with the cursor at different DPIs, check that the resulting physical width is bounded by `max(logical_width * 150/96, logical_width * 100/96)`.

## Possible Solutions & Risks

Listed from smallest to largest blast radius.

### Option A — Minimal: drop the WM_DPICHANGED path entirely when dragging; defer DPI change until release (preferred, smallest diff)

In `monitor_wnd_proc` WM_DPICHANGED branch (`src/platform/windows/mod.rs:559-587`):

- When `GetKeyState(VK_LBUTTON) & 0x8000 != 0`, **drop WM_DPICHANGED on the floor**: `return LRESULT(0)` (i.e. do not adopt suggested rect, do not forward to Slint). Windows will queue another WM_DPICHANGED after the drag releases (or the cursor fully crosses the boundary).
- Add `signals.dpi_recheck_pending = AtomicBool::new(false)`; set it inside the guarded branch. In `tick` step 6 (after drag releases), if set, re-evaluate: `app.window().set_size(PhysicalSize::new(width * ppp, height * ppp))` and `apply_window_pos`.
- **Add a one-time `app.window().set_scale_factor(...)` style call** if Slint exposes it; otherwise re-call `app.window().set_size` to force Slint to re-query DPI on next render.

Risks:
- Window will look at the wrong DPI until release. Acceptable: lyric window is small, users won't notice a few hundred ms of slightly larger/smaller text.
- May break Slint's internal font cache. Mitigation: clear cache by toggling `font-size` or by calling `set_font_family`/`set_font_size_px` again (already done each tick).
- If `SetWindowPos` inside the dropped branch is skipped, the window position may lag behind the cursor for ~16 ms. The drag system's `tick` already moves it the next frame, so net lag = 1 tick.

Diff size: ~20 lines inside `monitor_wnd_proc` + ~5 lines in `tick` + 1 signal flag. **Smallest complete fix.**

### Option B — Match cursor DPI by deferring resize until cursor monitor == window monitor

In the WM_DPICHANGED guard: only allow resize when the **window's monitor** (centroid) matches the cursor's monitor DPI. Compute window centre via `GetWindowRect` + `MonitorFromRect(MONITOR_DEFAULTTONEAREST)` and require `cursor_dpi_x == window_monitor_dpi`. Otherwise drop.

Risks: same as A but the heuristic is harder to reason about; still misses the straddle-by-bottom-edge case.

### Option C — Replace WndProc subclassing with Slint-native `on_scale_factor_changed`

Per `docs/dev-stage-3/review-finding.md` §8 recommendation. Requires verifying Slint 1.x API for `on_scale_factor_changed` (not present in all versions). May require upgrading Slint.

Risks: Medium. May introduce other regressions in winit/Skia integration. Larger diff (~50+ lines across multiple files).

### Option D — Forward WM_DPICHANGED to the original Slint WndProc so it can update `scale_factor`

Currently `monitor_wnd_proc` returns `LRESULT(0)` for WM_DPICHANGED without calling `CallWindowProcW(prev, …)`. Add a `CallWindowProcW(prev, hwnd, msg, wparam, lparam)` after the resize block.

Risks: Low. May double-apply the suggested rect (once by us, once by Slint's WndProc which usually also calls `SetWindowPos` to suggested rect). Could cause infinite loop if Slint's handler also fires our WndProc. **Verify Slint winit backend's WM_DPICHANGED behavior first.** May be the simplest fix if Slint's handler does the right thing.

### Recommendation

**Try Option D first** (3 lines), validate manually with the dual-monitor rig. If it causes the WM_DPICHANGED → Slint → another WndProc cascade, **fall back to Option A** (the explicit defer-until-release path). Option A is the smallest *correct* fix and has the lowest risk of reintroducing the same oscillation under a different stimulus (e.g. fast drag vs slow drag).

Either way, **separately** fix:
- `src/window/mod.rs:118-123` initial `set_size` should multiply by `app.window().scale_factor()` like `reload_config` does at line 368-371.
- `src/services/monitor.rs:124-145` `clamp_to_primary_monitor` should use the **target monitor's** `pixels_per_point` (currently always primary).

These are independent of the straddle bug but trip the same DPI corner.

## Supervisor coordination

No blocking decision. Two questions worth a sanity check before code:

1. Do you want the "drop WM_DPICHANGED during drag" fix (Option A) or the "forward to Slint" fix (Option D) as the primary strategy? My recommendation is **A**, but D is a 3-line patch worth attempting first.
2. Are you OK bundling the initial-size unit fix (`set_size` missing `* scale_factor`) into the same patch, or split into a separate diff?

I will proceed with the scout-only path (no file edits) and wait for direction.

## Start Here

Open `src/platform/windows/mod.rs:548-595` (`monitor_wnd_proc` WM_DPICHANGED branch). Read alongside `src/window/drag.rs:41-100` (`process_pointer_input`) and `src/window/mod.rs:269-329` (tick step 4 + step 5). Those three locations fully describe the straddle failure mode.