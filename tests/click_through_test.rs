/// Tests for the click-through (locked-window) state contract.
///
/// These tests verify that `Signals::is_locked` accurately reflects the
/// locked/unlocked state transitions that drive the `WM_NCHITTEST`
/// interception in `monitor_wnd_proc`.
///
/// Platform Win32 calls (`apply_locked_style`) cannot run in a
/// cross-platform CI environment, so these tests exercise the pure-Rust
/// signal contract independently.
use std::sync::atomic::Ordering;

use lyric_for_musicfox::context::Signals;

#[test]
fn is_locked_defaults_to_false() {
    let signals = Signals::default();
    assert!(!signals.is_locked.load(Ordering::Relaxed));
}

#[test]
fn is_locked_can_be_set_to_true() {
    let signals = Signals::default();
    signals.is_locked.store(true, Ordering::SeqCst);
    assert!(signals.is_locked.load(Ordering::Relaxed));
}

#[test]
fn is_locked_round_trips_locked_then_unlocked() {
    let signals = Signals::default();

    // Simulate apply_locked_style(locked = true)
    signals.is_locked.store(true, Ordering::SeqCst);
    assert!(signals.is_locked.load(Ordering::Relaxed), "should be locked after first lock");

    // Simulate apply_locked_style(locked = false)
    signals.is_locked.store(false, Ordering::SeqCst);
    assert!(!signals.is_locked.load(Ordering::Relaxed), "should be unlocked after unlock");
}

#[test]
fn is_locked_multiple_transitions() {
    let signals = Signals::default();

    for locked in [true, false, true, false, true] {
        signals.is_locked.store(locked, Ordering::SeqCst);
        assert_eq!(
            signals.is_locked.load(Ordering::Relaxed),
            locked,
            "signal should match after store({locked})"
        );
    }
}

#[test]
fn is_locked_independent_of_is_dragging() {
    let signals = Signals::default();
    signals.is_dragging.store(true, Ordering::SeqCst);
    signals.is_locked.store(true, Ordering::SeqCst);

    // Both flags are independent AtomicBools; toggling one must not affect the other.
    signals.is_dragging.store(false, Ordering::SeqCst);
    assert!(
        signals.is_locked.load(Ordering::Relaxed),
        "is_locked must remain true when only is_dragging changes"
    );

    signals.is_locked.store(false, Ordering::SeqCst);
    assert!(
        !signals.is_dragging.load(Ordering::Relaxed),
        "is_dragging must remain false when only is_locked changes"
    );
}
