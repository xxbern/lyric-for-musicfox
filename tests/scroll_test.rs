//! 静态排版几何与溢出截断测试

use lyric_for_musicfox::window::scroll::{compute_base_x, is_overflowing, LayoutState};

#[test]
fn test_not_overflowing_centers_horizontally() {
    assert!(!is_overflowing(100.0, 200.0));
    assert!(!is_overflowing(200.0, 200.0));
    assert_eq!(compute_base_x(100.0, 200.0), 50.0, "未超宽文本应水平居中");
    assert_eq!(compute_base_x(200.0, 200.0), 0.0, "宽度完全相等时从 0 对齐");
}

#[test]
fn test_overflowing_aligns_to_left_edge_for_clipping() {
    assert!(is_overflowing(200.1, 200.0));
    assert!(is_overflowing(500.0, 200.0));
    assert_eq!(
        compute_base_x(200.1, 200.0),
        0.0,
        "超宽文本必须靠左对齐，右侧超出部分由视口容器裁剪"
    );
    assert_eq!(
        compute_base_x(1200.0, 800.0),
        0.0,
        "超长歌词靠左对齐以展示首部并截断末尾"
    );
}

#[test]
fn test_layout_state_updates_correctly() {
    let mut state = LayoutState::new();
    assert!(state.needs_recompute);

    // 短文本
    let base_x = state.update(300.0, 800.0);
    assert_eq!(base_x, 250.0);
    assert_eq!(state.text_width, 300.0);
    assert_eq!(state.window_width, 800.0);
    assert!(!state.needs_recompute);

    // 变更为超长文本
    let base_x_overflow = state.update(1000.0, 800.0);
    assert_eq!(base_x_overflow, 0.0);
    assert_eq!(state.text_width, 1000.0);
}
