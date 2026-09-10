use lyric_for_musicfox::lyric::state::LyricState;
use lyric_for_musicfox::lyric::udp::{apply_to_state, is_lyric_update, parse_packet, RecvError};

#[test]
fn test_deserialization_complete() {
    let json = r#"{
        "type": "lyric_update",
        "playing": true,
        "time_ms": 12345,
        "volume": 85,
        "mode": "random",
        "song": {
            "id": 999,
            "name": "Song Name",
            "artist": "Artist Name",
            "album": "Album Name",
            "pic_url": "http://img.com",
            "duration": 200000,
            "is_favorite": true
        },
        "current_line": {
            "text": "Current Text",
            "words": []
        },
        "next_line": {
            "text": "Next Text",
            "words": []
        }
    }"#;
    let payload = parse_packet(json.as_bytes()).unwrap();
    assert_eq!(payload.type_, "lyric_update");
    assert!(payload.playing);
    assert_eq!(payload.time_ms, 12345);
    assert_eq!(payload.volume, 85);
    assert_eq!(payload.mode, "random");
    assert_eq!(payload.song.id, 999);
    assert_eq!(payload.song.name, "Song Name");
    assert_eq!(payload.song.artist, "Artist Name");
    assert_eq!(payload.song.album, "Album Name");
    assert_eq!(payload.song.pic_url, "http://img.com");
    assert_eq!(payload.song.duration, 200000);
    assert!(payload.song.is_favorite);
    assert_eq!(payload.current_line.text, "Current Text");
    assert_eq!(payload.next_line.text, "Next Text");
}

#[test]
fn test_deserialization_defaults() {
    let json = r#"{}"#;
    let payload = parse_packet(json.as_bytes()).unwrap();
    assert_eq!(payload.type_, "lyric_update");
    assert!(!payload.playing);
    assert_eq!(payload.time_ms, 0);
    assert_eq!(payload.volume, 0);
    assert_eq!(payload.mode, "");
    assert_eq!(payload.song.id, 0);
    assert_eq!(payload.song.name, "");
    assert_eq!(payload.song.artist, "");
    assert_eq!(payload.song.album, "");
    assert_eq!(payload.song.pic_url, "");
    assert_eq!(payload.song.duration, 0);
    assert!(!payload.song.is_favorite);
    assert_eq!(payload.current_line.text, "");
    assert!(payload.current_line.words.is_empty());
    assert_eq!(payload.next_line.text, "");
    assert!(payload.next_line.words.is_empty());
}

#[test]
fn test_is_lyric_update() {
    let mut payload = parse_packet(b"{}").unwrap();
    assert!(is_lyric_update(&payload));

    payload.type_ = "other_type".to_string();
    assert!(!is_lyric_update(&payload));
}

#[test]
fn test_parse_packet_limits() {
    // Exactly 8192 should parse (though it is invalid JSON in this case, returning Parse error, not TooLarge)
    let max_buf = vec![b' '; 8192];
    match parse_packet(&max_buf) {
        Err(RecvError::Parse(_)) => {}
        other => panic!(
            "Expected Parse error on whitespace-only 8192 bytes, got {:?}",
            other
        ),
    }

    // 8193 bytes should return TooLarge
    let oversize_buf = vec![b' '; 8193];
    match parse_packet(&oversize_buf) {
        Err(RecvError::TooLarge { len }) => {
            assert_eq!(len, 8193);
        }
        other => panic!("Expected TooLarge, got {:?}", other),
    }
}

#[test]
fn test_apply_to_state() {
    let json = r#"{
        "playing": true,
        "time_ms": 54321,
        "song": {
            "name": "My Song",
            "artist": "My Artist"
        },
        "current_line": {
            "text": "Hello World"
        },
        "next_line": {
            "text": "Hello Next"
        }
    }"#;
    let payload = parse_packet(json.as_bytes()).unwrap();

    let mut state = LyricState::placeholder();
    assert!(state.current_line.is_placeholder);
    assert_eq!(*state.current_line.text, *"......");

    apply_to_state(&mut state, &payload);

    assert!(state.playing);
    assert_eq!(state.time_ms, 54321);
    assert_eq!(state.song_name, "My Song");
    assert_eq!(state.song_artist, "My Artist");
    assert_eq!(*state.current_line.text, *"Hello World");
    assert_eq!(*state.next_line.text, *"Hello Next");
    assert!(!state.current_line.is_placeholder);
}

#[test]
fn test_apply_to_state_placeholder_on_empty() {
    let json = r#"{
        "current_line": {
            "text": ""
        }
    }"#;
    let payload = parse_packet(json.as_bytes()).unwrap();
    let mut state = LyricState::placeholder();
    state.current_line.is_placeholder = false;

    apply_to_state(&mut state, &payload);
    assert!(state.current_line.is_placeholder);
    assert_eq!(*state.current_line.text, *"");
}

#[test]
fn test_udp_event_bus_notifies_lyric_state_changed() {
    use std::sync::Arc;
    use lyric_for_musicfox::context::AppContext;
    use lyric_for_musicfox::event_bus::{AppEvent, EventBus};
    use lyric_for_musicfox::Config;

    let (bus, rx) = EventBus::new(16);
    let ctx = Arc::new(AppContext::new(Config::default(), LyricState::placeholder(), bus));

    let payload = parse_packet(br#"{"playing":true,"current_line":{"text":"New Line"}}"#).unwrap();
    {
        let mut state_guard = ctx.state.write().unwrap();
        let prev_text = state_guard.current_line.text.clone();
        let prev_playing = state_guard.playing;
        apply_to_state(&mut state_guard, &payload);
        if prev_text != state_guard.current_line.text || prev_playing != state_guard.playing {
            ctx.event_bus.emit(AppEvent::LyricStateChanged);
            ctx.event_bus.emit(AppEvent::RequestRepaint);
        }
    }

    let mut received_state_changed = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, AppEvent::LyricStateChanged) {
            received_state_changed = true;
        }
    }
    assert!(received_state_changed, "歌词更新必须派发 LyricStateChanged 事件");
}
