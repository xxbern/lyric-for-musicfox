use lyric_for_musicfox::context::Signals as SharedSignals;
use lyric_for_musicfox::lyric::state::LyricState;
use lyric_for_musicfox::window::drag::{
    compute_new_position, flush_on_exit, process_pointer_input, DragPhase, DragState,
};

#[test]
fn compute_new_position_no_delta() {
    assert_eq!(
        compute_new_position((100, 100), (200, 200), (100, 100)),
        (200, 200)
    );
}

#[test]
fn compute_new_position_with_delta() {
    assert_eq!(
        compute_new_position((100, 100), (200, 200), (150, 130)),
        (250, 230)
    );
}

#[test]
fn negative_position_allowed() {
    assert_eq!(
        compute_new_position((-100, -100), (-200, -200), (-50, -50)),
        (-150, -150)
    );
}

#[test]
fn drag_state_starts_idle() {
    let s = DragState::new();
    assert_eq!(s.phase, DragPhase::Idle);
}

#[test]
fn pressed_when_locked_stays_idle() {
    let mut drag = DragState::new();
    let mut state = LyricState::placeholder();
    let signals = SharedSignals::default();
    let moved = process_pointer_input(
        &mut drag,
        &mut state,
        &signals,
        true,
        Some((10, 10)),
        true,
        false,
        (100, 100),
    );
    assert!(moved.is_none());
    assert_eq!(drag.phase, DragPhase::Idle);
}

#[test]
fn idle_to_active_on_pressed_when_unlocked() {
    let mut drag = DragState::new();
    let mut state = LyricState::placeholder();
    let signals = SharedSignals::default();
    let moved = process_pointer_input(
        &mut drag,
        &mut state,
        &signals,
        false,
        Some((10, 10)),
        true,
        false,
        (100, 200),
    );
    assert_eq!(moved, Some((100, 200)));
    assert_eq!(
        drag.phase,
        DragPhase::Active {
            anchor_physical: (10, 10),
            anchor_window: (100, 200),
        }
    );
}

#[test]
fn second_press_does_not_reset_anchor() {
    let mut drag = DragState::new();
    let mut state = LyricState::placeholder();
    let signals = SharedSignals::default();
    let _ = process_pointer_input(
        &mut drag,
        &mut state,
        &signals,
        false,
        Some((10, 10)),
        true,
        false,
        (100, 200),
    );
    let moved = process_pointer_input(
        &mut drag,
        &mut state,
        &signals,
        false,
        Some((40, 50)),
        true,
        false,
        (100, 200),
    );
    assert_eq!(moved, Some((130, 240)));
    assert_eq!(
        drag.phase,
        DragPhase::Active {
            anchor_physical: (10, 10),
            anchor_window: (100, 200),
        }
    );
}

#[test]
fn release_writes_state_and_returns_idle() {
    let mut drag = DragState::new();
    let mut state = LyricState::placeholder();
    let signals = SharedSignals::default();
    let _ = process_pointer_input(
        &mut drag,
        &mut state,
        &signals,
        false,
        Some((10, 10)),
        true,
        false,
        (100, 200),
    );
    let moved = process_pointer_input(
        &mut drag,
        &mut state,
        &signals,
        false,
        Some((40, 50)),
        false,
        true,
        (100, 200),
    );
    assert_eq!(moved, Some((130, 240)));
    assert_eq!(state.pos_x, 130);
    assert_eq!(state.pos_y, 240);
    assert_eq!(drag.phase, DragPhase::Idle);
}

#[test]
fn flush_on_exit_syncs_active_position() {
    let mut drag = DragState {
        phase: DragPhase::Active {
            anchor_physical: (0, 0),
            anchor_window: (1, 1),
        },
    };
    let mut state = LyricState::placeholder();
    let signals = SharedSignals::default();
    flush_on_exit(&mut drag, &mut state, &signals, (77, 88));
    assert_eq!(state.pos_x, 77);
    assert_eq!(state.pos_y, 88);
    assert_eq!(drag.phase, DragPhase::Idle);
}
