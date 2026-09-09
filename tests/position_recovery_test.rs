use lyric_for_musicfox::context::AppContext;
use lyric_for_musicfox::event_bus::EventBus;
use lyric_for_musicfox::lyric::state::LyricState;
use lyric_for_musicfox::services::monitor::{clamp_to_primary_monitor, MonitorLayout, MonitorRect};
use lyric_for_musicfox::services::position::PositionService;
use lyric_for_musicfox::window::drag::{process_pointer_input, DragState};
use lyric_for_musicfox::Config;
use std::sync::Arc;

fn make_ctx_with_pos(pos_x: i32, pos_y: i32) -> (Arc<AppContext>, PositionService) {
    let mut cfg = Config::default();
    cfg.window.pos_x = Some(pos_x);
    cfg.window.pos_y = Some(pos_y);
    cfg.window.width = 150;
    cfg.window.height = 60;

    let (bus, _) = EventBus::new(16);
    let mut state = LyricState::placeholder();
    state.pos_x = pos_x;
    state.pos_y = pos_y;
    state.target_pos_x = pos_x;
    state.target_pos_y = pos_y;

    let ctx = Arc::new(AppContext::new(cfg, state, bus));
    let pos_svc = PositionService::new(ctx.clone());
    (ctx, pos_svc)
}

fn make_layout(width: i32, height: i32) -> MonitorLayout {
    let rect = MonitorRect {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    MonitorLayout {
        monitors: vec![rect],
        primary: 0,
        primary_monitor: rect,
    }
}

#[test]
fn test_dual_track_initialization() {
    // 验证冷启动配置加载后，意图坐标与当前坐标双轨正确初始化
    let (_ctx, pos_svc) = make_ctx_with_pos(1500, -15);
    assert_eq!(pos_svc.get_target_pos(), (1500, -15));
    assert_eq!(pos_svc.get_current_pos(), (1500, -15));
}

#[test]
fn test_lifecycle_abnormal_shrink_and_auto_recovery() {
    // 模拟完整生命周期：
    // 1. 用户正常意图 (1500, -15)
    let (_ctx, pos_svc) = make_ctx_with_pos(1500, -15);
    let win_size = (150, 60);

    // 2. 模拟非瞬态小屏幕拓扑（如外接屏幕被切换到一个物理 1280x720 分辨率）
    let shrink_layout = make_layout(1280, 720);
    let target = pos_svc.get_target_pos();

    // 投影计算出避险位置 (1280 - 150 = 1130, -15 垂直仍在 720 范围内保留)
    let clamped_avoid = clamp_to_primary_monitor(target, &shrink_layout, win_size);
    assert_eq!(clamped_avoid, (1130, -15));

    // 系统将窗口临时移到避险位置，更新当前坐标，但严禁改动 target_pos
    pos_svc.set_current_pos(clamped_avoid);
    assert_eq!(pos_svc.get_current_pos(), (1130, -15));
    assert_eq!(pos_svc.get_target_pos(), (1500, -15), "避险裁剪严禁篡改用户意图坐标");

    // 3. 屏幕恢复到原始 1920x1080 拓扑
    let restored_layout = make_layout(1920, 1080);

    // 下一轮巡检：以 target_pos 为基准重新投影
    let target_now = pos_svc.get_target_pos();
    let clamped_restored = clamp_to_primary_monitor(target_now, &restored_layout, win_size);

    // 断言：计算结果自动还原为 (1500, -15)！
    assert_eq!(clamped_restored, (1500, -15), "屏幕恢复后基于意图坐标自动精确归位");

    // 更新视口，完成自愈
    pos_svc.set_current_pos(clamped_restored);
    assert_eq!(pos_svc.get_current_pos(), (1500, -15));
}

#[test]
fn test_user_drag_updates_target_intent() {
    // 验证用户在解锁状态下主动拖拽，松开时同步更新 target_pos
    let (ctx, _pos_svc) = make_ctx_with_pos(100, 100);
    let mut drag = DragState::new();

    // 模拟按下鼠标
    {
        let mut guard = ctx.state.write().unwrap();
        let _ = process_pointer_input(
            &mut drag,
            &mut guard,
            &ctx.signals,
            false, // unlocked
            Some((110, 110)),
            true, // left_pressed
            false,
            (100, 100),
        );
    }
    assert!(ctx.signals.is_dragging.load(std::sync::atomic::Ordering::SeqCst));

    // 模拟移动并松开鼠标于 (200, 250)
    let final_pos = {
        let mut guard = ctx.state.write().unwrap();
        process_pointer_input(
            &mut drag,
            &mut guard,
            &ctx.signals,
            false,
            Some((210, 260)),
            false,
            true, // left_released
            (100, 100),
        )
    };

    assert_eq!(final_pos, Some((200, 250)));
    let guard = ctx.state.read().unwrap();
    assert_eq!(guard.pos_x, 200);
    assert_eq!(guard.pos_y, 250);
    assert_eq!(guard.target_pos_x, 200, "拖拽释放必须确认为新意图坐标");
    assert_eq!(guard.target_pos_y, 250, "拖拽释放必须确认为新意图坐标");
}

#[test]
fn test_idle_tick_never_causes_ghost_movement() {
    // 模拟后台静默运行：多次巡检 tick，坐标稳定不变
    let (_ctx, pos_svc) = make_ctx_with_pos(1500, -15);
    let layout = make_layout(1920, 1080);
    let win_size = (150, 60);

    for _ in 0..100 {
        let target = pos_svc.get_target_pos();
        let current = pos_svc.get_current_pos();
        let clamped = clamp_to_primary_monitor(target, &layout, win_size);
        if clamped != current {
            pos_svc.set_current_pos(clamped);
        }
    }

    assert_eq!(pos_svc.get_target_pos(), (1500, -15));
    assert_eq!(pos_svc.get_current_pos(), (1500, -15));
}

#[test]
fn test_zero_origin_dual_track_preserved() {
    // 验证原点 (0, 0) 是合法意图坐标，即使发生避险移位，其 target_pos 仍为 (0, 0)
    let (_ctx, pos_svc) = make_ctx_with_pos(0, 0);
    assert_eq!(pos_svc.get_target_pos(), (0, 0));
    assert_eq!(pos_svc.get_current_pos(), (0, 0));

    // 避险移位
    pos_svc.set_current_pos((100, 100));
    assert_eq!(pos_svc.get_current_pos(), (100, 100));
    assert_eq!(pos_svc.get_target_pos(), (0, 0), "合法原点 (0, 0) 意图绝不因避险而丢失");
}

#[test]
fn test_drag_moving_does_not_mutate_target_before_release() {
    // 验证按住拖拽移动中（未松开），不会覆盖意图坐标
    let (ctx, _pos_svc) = make_ctx_with_pos(100, 100);
    let mut drag = DragState::new();

    // 按下
    {
        let mut guard = ctx.state.write().unwrap();
        let _ = process_pointer_input(
            &mut drag,
            &mut guard,
            &ctx.signals,
            false,
            Some((110, 110)),
            true,
            false,
            (100, 100),
        );
    }

    // 移动中（未 release）
    let moving_pos = {
        let mut guard = ctx.state.write().unwrap();
        process_pointer_input(
            &mut drag,
            &mut guard,
            &ctx.signals,
            false,
            Some((150, 180)),
            false,
            false, // 未释放！
            (100, 100),
        )
    };

    assert_eq!(moving_pos, Some((140, 170)));
    let guard = ctx.state.read().unwrap();
    // 移动中未松手：target_pos 仍应是最初的 100, 100！
    assert_eq!(guard.target_pos_x, 100);
    assert_eq!(guard.target_pos_y, 100);
}
