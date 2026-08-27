use lyric_for_musicfox::services::monitor::{
    center_on_primary_monitor, clamp_to_primary_monitor, MonitorLayout, MonitorRect,
};

fn make_layout(mon: MonitorRect) -> MonitorLayout {
    MonitorLayout {
        monitors: vec![mon],
        primary: 0,
        primary_monitor: mon,
    }
}

#[test]
fn clamp_in_range_no_change() {
    let layout = make_layout(MonitorRect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    });
    let clamped = clamp_to_primary_monitor((500, 300), &layout, (800, 80));
    assert_eq!(clamped, (500, 300));
}

#[test]
fn clamp_right_overflow_to_monitor_minus_win() {
    let layout = make_layout(MonitorRect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    });
    let clamped = clamp_to_primary_monitor((2000, 500), &layout, (800, 80));
    assert_eq!(clamped, (1120, 500));
}

#[test]
fn clamp_negative_x_allowed_past_left() {
    let layout = make_layout(MonitorRect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    });
    let clamped = clamp_to_primary_monitor((-500, 500), &layout, (800, 80));
    assert_eq!(clamped, (-500, 500));
}

#[test]
fn center_uses_work_area_top_plus_100_dip() {
    let layout = make_layout(MonitorRect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    });
    let (x, y) = center_on_primary_monitor(&layout, (800, 80), 1.0);
    assert_eq!(x, 560);
    assert_eq!(y, 100);
}

#[test]
fn center_with_150_percent_dpi() {
    let layout = make_layout(MonitorRect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    });
    let (x, y) = center_on_primary_monitor(&layout, (1200, 120), 1.5);
    let _ = x;
    assert_eq!(y, 150);
}

#[test]
fn clamp_keeps_secondary_when_still_connected() {
    let layout = MonitorLayout {
        monitors: vec![
            MonitorRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            MonitorRect {
                left: 1920,
                top: 0,
                right: 3840,
                bottom: 1080,
            },
        ],
        primary: 0,
        primary_monitor: MonitorRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        },
    };
    let clamped = clamp_to_primary_monitor((2000, 500), &layout, (800, 80));
    assert_eq!(clamped, (2000, 500));
}

#[test]
fn clamp_keeps_negative_when_left_monitor_connected() {
    let layout = MonitorLayout {
        monitors: vec![
            MonitorRect {
                left: -1920,
                top: 0,
                right: 0,
                bottom: 1080,
            },
            MonitorRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
        ],
        primary: 1,
        primary_monitor: MonitorRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        },
    };
    let clamped = clamp_to_primary_monitor((-1500, 200), &layout, (800, 80));
    assert_eq!(clamped, (-1500, 200));
}

#[test]
fn clamp_fully_offscreen_after_unplug_moves_onto_primary() {
    let layout = make_layout(MonitorRect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    });
    let clamped = clamp_to_primary_monitor((-2000, 500), &layout, (800, 80));
    assert_eq!(clamped, (0, 500));
}
