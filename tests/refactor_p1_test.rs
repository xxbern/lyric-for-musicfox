use lyric_for_musicfox::config::Config;
use lyric_for_musicfox::context::AppContext;
use lyric_for_musicfox::event_bus::{AppEvent, EventBus};
use lyric_for_musicfox::lyric::state::LyricState;
use std::sync::Arc;

#[test]
fn test_event_bus_capacity_limit_under_high_load() {
    // Create an EventBus with a tiny capacity of 3
    let (event_bus, rx) = EventBus::new(3);

    // Emit 3 events successfully
    event_bus.emit(AppEvent::RequestRepaint);
    event_bus.emit(AppEvent::RequestRepaint);
    event_bus.emit(AppEvent::RequestRepaint);

    // This 4th emit exceeds capacity.
    // In block/try_send semantics, it should warn/log and return immediately without blocking.
    let start = std::time::Instant::now();
    event_bus.emit(AppEvent::RequestRepaint);
    let elapsed = start.elapsed();

    // Ensure it did not block (usually blocking crossbeam bounded channel takes longer/hangs)
    assert!(
        elapsed.as_millis() < 50,
        "Event Bus emit blocked under high load!"
    );

    // Validate that we can read exactly 3 events from the receiver
    let mut count = 0;
    while let Ok(AppEvent::RequestRepaint) = rx.try_recv() {
        count += 1;
    }
    assert_eq!(count, 3);
}

#[test]
fn test_config_reloaded_dispatches_correctly() {
    let default_config = Config::default();
    let (event_bus, rx) = EventBus::new(10);

    let ctx = Arc::new(AppContext::new(
        default_config.clone(),
        LyricState::placeholder(),
        event_bus,
    ));

    // Update config with a new window width
    let mut new_config = default_config.clone();
    new_config.window.width = 1234;

    ctx.update_config(new_config.clone());

    // Verify the context returns the updated config
    let retrieved_config = ctx.get_config();
    assert_eq!(retrieved_config.window.width, 1234);

    // Verify the EventBus received the ConfigReloaded event
    let event = rx
        .try_recv()
        .expect("Expected to receive an AppEvent from update_config");
    match event {
        AppEvent::ConfigReloaded(cfg) => {
            assert_eq!(cfg.window.width, 1234);
        }
        _ => panic!("Expected AppEvent::ConfigReloaded variant"),
    }
}
