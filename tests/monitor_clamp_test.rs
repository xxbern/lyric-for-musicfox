use lyric_for_musicfox::services::monitor::{
    clamp_to_primary_monitor, is_transient_layout, window_intersects_monitor, MonitorLayout,
    MonitorRect,
};

fn make_single_layout(left: i32, top: i32, width: i32, height: i32) -> MonitorLayout {
    let rect = MonitorRect {
        left,
        top,
        right: left + width,
        bottom: top + height,
    };
    MonitorLayout {
        monitors: vec![rect],
        primary: 0,
        primary_monitor: rect,
    }
}

#[test]
fn test_negative_y_tolerance_maintained_when_partially_visible() {
    // 真实场景：用户配置 pos_x = 1500, pos_y = -15，尺寸 (150, 60)
    // 屏幕 1920x1080，窗口顶部溢出 15px，但在垂直方向有 45px 留在屏幕内
    let layout = make_single_layout(0, 0, 1920, 1080);
    let pos = (1500, -15);
    let win_size = (150, 60);

    assert!(window_intersects_monitor(pos, win_size, &layout.primary_monitor));

    let clamped = clamp_to_primary_monitor(pos, &layout, win_size);
    assert_eq!(clamped, (1500, -15), "负坐标 -15 在有部分可见时必须原样保留，严禁强制归零");
}

#[test]
fn test_transient_display_skips_clamping() {
    // 模拟 Windows 息屏/断联产生的 1024x768 瞬态小屏
    let transient_layout = make_single_layout(0, 0, 1024, 768);
    assert!(is_transient_layout(&transient_layout));

    // 用户在原本 1080p 屏幕配置了 (1500, -15)
    // 在 1024x768 下若无瞬态保护，会被强行裁剪到 874, 0
    let pos = (1500, -15);
    let win_size = (150, 60);

    let clamped = clamp_to_primary_monitor(pos, &transient_layout, win_size);
    assert_eq!(clamped, (1500, -15), "检测到瞬态小屏幕时必须跳过破坏性裁剪，保护原始意图坐标");
}

#[test]
fn test_transient_display_800x600_skips_clamping() {
    // 更极端的断联瞬态：800x600 单屏
    let transient_layout = make_single_layout(0, 0, 800, 600);
    assert!(is_transient_layout(&transient_layout));

    let pos = (1500, -15);
    let win_size = (150, 60);
    let clamped = clamp_to_primary_monitor(pos, &transient_layout, win_size);
    assert_eq!(clamped, (1500, -15));
}

#[test]
fn test_horizontal_clamp_preserves_negative_y_intent() {
    // 真实拔屏场景：副屏移除后，窗口在 x=2500, y=-15
    // 水平方向完全脱离主屏（1920宽），拉回主屏边缘 (1920 - 150 = 1770)
    // 但垂直方向原本就与主屏相交（-15 到 45），拉回时必须保留 y = -15！
    let layout = make_single_layout(0, 0, 1920, 1080);
    let pos = (2500, -15);
    let win_size = (150, 60);

    let clamped = clamp_to_primary_monitor(pos, &layout, win_size);
    assert_eq!(clamped, (1770, -15), "水平拉回主屏时不得连带破坏垂直合法负坐标");
}

#[test]
fn test_vertical_clamp_preserves_valid_x_intent() {
    // 窗口完全掉落到屏幕下方 (x=1500, y=1200)，主屏高 1080
    // 垂直拉回边缘 (1080 - 60 = 1020)，但水平方向原本就在 1500，必须保留 x = 1500
    let layout = make_single_layout(0, 0, 1920, 1080);
    let pos = (1500, 1200);
    let win_size = (150, 60);

    let clamped = clamp_to_primary_monitor(pos, &layout, win_size);
    assert_eq!(clamped, (1500, 1020), "垂直拉回主屏时不得破坏原本合法的水平坐标");
}

#[test]
fn test_normal_resolution_single_screen_not_transient() {
    // 普通 1080p, 2K, 4K 单屏绝不是瞬态
    let layout_1080p = make_single_layout(0, 0, 1920, 1080);
    let layout_2k = make_single_layout(0, 0, 2560, 1440);
    let layout_4k = make_single_layout(0, 0, 3840, 2160);

    assert!(!is_transient_layout(&layout_1080p));
    assert!(!is_transient_layout(&layout_2k));
    assert!(!is_transient_layout(&layout_4k));
}

#[test]
fn test_multi_monitor_not_transient() {
    // 哪怕副屏很小，只要有多显示器，就不是息屏断联的单屏瞬态
    let multi_layout = MonitorLayout {
        monitors: vec![
            MonitorRect { left: 0, top: 0, right: 1024, bottom: 768 },
            MonitorRect { left: 1024, top: 0, right: 2048, bottom: 768 },
        ],
        primary: 0,
        primary_monitor: MonitorRect { left: 0, top: 0, right: 1024, bottom: 768 },
    };
    assert!(!is_transient_layout(&multi_layout));
}

#[test]
fn test_diagonal_complete_offscreen_clamp() {
    // 双轴同时完全出界：(2500, 1200)
    let layout = make_single_layout(0, 0, 1920, 1080);
    let pos = (2500, 1200);
    let win_size = (150, 60);

    let clamped = clamp_to_primary_monitor(pos, &layout, win_size);
    assert_eq!(clamped, (1770, 1020), "对角线完全出界时双轴均安全拉回主屏边缘");
}

#[test]
fn test_completely_offscreen_negative_y_clamped_to_top() {
    // 顶部完全出界：pos.1 = -100，窗口高度 60（bottom = -40 <= 0）
    let layout = make_single_layout(0, 0, 1920, 1080);
    let pos = (1500, -100);
    let win_size = (150, 60);

    let clamped = clamp_to_primary_monitor(pos, &layout, win_size);
    assert_eq!(clamped, (1500, 0), "完全脱离顶部的窗口拉回顶边 0，同时保留水平 x 意图");
}

#[test]
fn test_left_negative_x_tolerance_maintained() {
    // 左侧微负坐标溢出保留：pos.0 = -15，pos.1 = 500，窗口宽度 150（right = 135 > 0）
    let layout = make_single_layout(0, 0, 1920, 1080);
    let pos = (-15, 500);
    let win_size = (150, 60);

    let clamped = clamp_to_primary_monitor(pos, &layout, win_size);
    assert_eq!(clamped, (-15, 500), "左侧轻微负坐标溢出保留用户意图");
}

#[test]
fn test_oversized_window_aligned_to_top_left() {
    // 窗口尺寸大于显示器尺寸场景：win_w = 2500 > 1920，win_h = 1200 > 1080
    let layout = make_single_layout(0, 0, 1920, 1080);
    let pos = (3000, 2000);
    let win_size = (2500, 1200);

    let clamped = clamp_to_primary_monitor(pos, &layout, win_size);
    assert_eq!(clamped, (0, 0), "超大窗口拉回应左上对齐，避免推入负坐标导致内容不可见");
}

#[test]
fn test_empty_or_vertical_transient_displays() {
    // 空显示器列表视为瞬态
    let empty_layout = MonitorLayout {
        monitors: vec![],
        primary: 0,
        primary_monitor: MonitorRect { left: 0, top: 0, right: 1920, bottom: 1080 },
    };
    assert!(is_transient_layout(&empty_layout));

    // 竖屏 768x1024 瞬态
    let vertical_layout = make_single_layout(0, 0, 768, 1024);
    assert!(is_transient_layout(&vertical_layout));
}
