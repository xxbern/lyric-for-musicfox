//! 横向跑马灯状态机测试
//!
//! 覆盖 Oracle 计划：
//! - 不超宽不滚
//! - `30.0 * dt` 位移
//! - 文本变化重置到 window_width
//! - `offset_x <= -text_width` wrap 到 window_width
//! - 字体变更重置
//! - resize 行为

use lyric_for_musicfox::window::scroll::{ScrollState, SCROLL_SPEED_DIP_PER_SECOND};

#[test]
fn no_scroll_when_text_fits_window() {
    assert!(!ScrollState::should_scroll(100.0, 200.0));
    assert!(!ScrollState::should_scroll(200.0, 200.0));
    assert!(ScrollState::should_scroll(200.0001, 200.0));
}

#[test]
fn dt_drives_offset_leftward_at_constant_speed() {
    let mut s = ScrollState::new();
    // 1000 px 文本在 200 px 窗口中
    s.on_text_changed(1000.0, 200.0);
    assert_eq!(s.offset_x, 200.0, "起点应为 window_width");

    // 一帧 0.5 秒
    s.update(0.5, "long", 200.0);
    let expected = 200.0 - SCROLL_SPEED_DIP_PER_SECOND * 0.5;
    assert!(
        (s.offset_x - expected).abs() < 1e-3,
        "offset_x 应推进 30*dt；got {}",
        s.offset_x
    );
}

#[test]
fn text_change_resets_offset_to_window_width_when_still_overflow() {
    let mut s = ScrollState::new();
    s.on_text_changed(1000.0, 200.0);
    s.update(1.0, "long", 200.0);
    // 此时 offset_x 已经离开起点
    assert!(s.offset_x < 200.0);

    // 文本变化 + 仍超宽
    s.update(0.0, "different_long_text", 200.0);
    // last_text 已变 → on_text_changed 触发
    assert_eq!(s.offset_x, 200.0, "文本变化后 offset_x 应回到 window_width");
}

#[test]
fn text_change_to_short_centers_and_stops_scrolling() {
    let mut s = ScrollState::new();
    s.on_text_changed(1000.0, 200.0);
    // renderer 先更新 self.text_width 为新测宽度（短文本），再调 update
    s.text_width = 50.0;
    s.update(0.0, "hi", 200.0);
    assert_eq!(s.offset_x, 0.0, "不再超宽时归零");
}

#[test]
fn wrap_offset_resets_when_left_edge_fully_out() {
    let wrapped = ScrollState::wrap_offset_if_needed(
        -1000.5, // offset_x <= -text_width (1000)
        1000.0, 200.0,
    );
    assert_eq!(wrapped, 200.0, "应回到 window_width = 200");
    let not_yet = ScrollState::wrap_offset_if_needed(-999.999, 1000.0, 200.0);
    assert_eq!(not_yet, -999.999, "未达终点不 wrap");
}

#[test]
fn full_scroll_cycle_reaches_end_and_wraps() {
    let mut s = ScrollState::new();
    s.on_text_changed(1000.0, 200.0); // start at 200
                                      // 推进足够长时间让 offset_x 跨过 -1000
    let total_time = (200.0 + 1000.0) / SCROLL_SPEED_DIP_PER_SECOND + 1.0; // +1s 安全裕量
    let steps = 100;
    let dt = total_time / steps as f32;
    for _ in 0..steps {
        s.update(dt, "scroll", 200.0);
    }
    // 任意时刻 offset_x ∈ [window_width, -text_width]
    assert!(
        s.offset_x >= -1000.0 - f32::EPSILON && s.offset_x <= 200.0 + f32::EPSILON,
        "offset_x 越界: {}",
        s.offset_x
    );
}

#[test]
fn font_change_reset_resets_offset_when_text_overflows() {
    let mut s = ScrollState::new();
    s.on_text_changed(1000.0, 200.0);
    s.update(1.0, "long", 200.0);
    assert!(s.offset_x < 200.0);

    s.reset_for_font_change(1100.0, 200.0);
    assert_eq!(s.offset_x, 200.0, "字体变更后偏移应重置");
}

#[test]
fn resize_below_text_width_keeps_offset() {
    let mut s = ScrollState::new();
    s.on_text_changed(1000.0, 200.0);
    let before = s.offset_x;
    // 缩窄到 150（仍超宽）
    s.update(0.1, "long", 150.0);
    // last_text 未变；width 变化 → 不强制重置
    assert!(s.offset_x < 200.0);
    assert!(s.offset_x < before + 1.0);
}

#[test]
fn resize_above_text_width_centers() {
    let mut s = ScrollState::new();
    s.on_text_changed(1000.0, 200.0);
    // 拉宽到 1500
    s.update(0.0, "long", 1500.0);
    assert_eq!(s.offset_x, 0.0, "不再超宽时居中");
}
