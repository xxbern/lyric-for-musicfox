#[cfg(windows)]
mod windows_tests {
    use lyric_for_musicfox::config::Config;
    use lyric_for_musicfox::lyric::state::LyricState;
    use lyric_for_musicfox::pipe::{get_pipe_name, get_session_id, pos, reload};
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex, RwLock};
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_pipe_name_generation() {
        let sid = get_session_id();
        let name = get_pipe_name("test", sid);
        assert!(name.contains("lyric-for-musicfox-test-"));
    }

    #[test]
    fn test_reload_pipe_communication() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");

        let default_config = Config::default();
        lyric_for_musicfox::save_config(&default_config, &config_path).unwrap();

        let (event_bus, rx) = lyric_for_musicfox::event_bus::EventBus::new(8);
        let ctx = Arc::new(lyric_for_musicfox::context::AppContext::new(
            default_config,
            LyricState::placeholder(),
            event_bus,
        ));

        reload::start_server(config_path, ctx.clone());
        std::thread::sleep(Duration::from_millis(100));

        reload::notify_reload();

        let mut success = false;
        for _ in 0..20 {
            while let Ok(event) = rx.try_recv() {
                if let lyric_for_musicfox::event_bus::AppEvent::ConfigReloaded(_) = event {
                    success = true;
                }
            }
            if success {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(success);
    }

    #[test]
    fn test_pos_pipe_communication() {
        let default_config = Config::default();
        let (event_bus, _rx) = lyric_for_musicfox::event_bus::EventBus::new(8);
        let ctx = Arc::new(lyric_for_musicfox::context::AppContext::new(
            default_config,
            LyricState::placeholder(),
            event_bus,
        ));
        {
            let mut s = ctx.state.write().unwrap();
            s.pos_x = 987;
            s.pos_y = 654;
        }

        pos::start_server(ctx.clone());
        std::thread::sleep(Duration::from_millis(100));

        let res = pos::query_pos();
        assert_eq!(res, Some((987, 654)));
    }
}

#[cfg(not(windows))]
mod stub_tests {
    use lyric_for_musicfox::config::Config;
    use lyric_for_musicfox::lyric::state::LyricState;
    use lyric_for_musicfox::pipe::{get_pipe_name, get_session_id, pos, presence, reload};
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn test_stub_functions_run_successfully() {
        let sid = get_session_id();
        assert_eq!(sid, 0);
        let name = get_pipe_name("test", sid);
        assert_eq!(name, "lyric-for-musicfox-test-0");

        let default_config = Config::default();
        let (event_bus, _rx) = lyric_for_musicfox::event_bus::EventBus::new(8);
        let ctx = Arc::new(lyric_for_musicfox::context::AppContext::new(
            default_config,
            LyricState::placeholder(),
            event_bus,
        ));

        reload::start_server(PathBuf::from("config.toml"), ctx.clone());
        reload::notify_reload();

        pos::start_server(ctx);
        assert_eq!(pos::query_pos(), None);

        presence::start_server();
        assert!(presence::try_activate_existing().is_err());
    }
}
