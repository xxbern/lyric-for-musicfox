use log::LevelFilter;
use lyric_for_musicfox::logger::{self, error, info, warn};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_logger_all() {
    // Ensure clean env for the first part
    std::env::remove_var("LYRIC_LOG");

    let dir = tempdir().unwrap();
    let log_path = dir.path().join("log.txt");
    let log_path_old = dir.path().join("log.txt.old");

    // PART 1: test rotation and drain
    {
        let _guard = logger::init(&log_path, LevelFilter::Info, true);

        info!("Hello, world!");

        // Push a flush command to block until the log is processed
        log::logger().flush();

        // Check if log file exists and contains the string
        assert!(log_path.exists());
        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Hello, world!"));

        // Now push a massive log record to trigger rotate: 5MB+
        let large_msg = "A".repeat(5 * 1024 * 1024 + 100);
        warn!("{}", large_msg);

        // Flush to block until background thread finishes processing
        log::logger().flush();

        // Check that rotation occurred
        assert!(log_path_old.exists());
        // A new empty/small log file should have been created
        assert!(log_path.exists());

        // Write another message to new file
        error!("New file message");
    }

    // LoggerGuard dropped now. Check that the background thread finished draining
    let content = fs::read_to_string(&log_path).unwrap();
    assert!(content.contains("New file message"));

    // PART 2: test logger filter and env
    std::env::set_var("LYRIC_LOG", "WARN");

    let log_path2 = dir.path().join("log2.txt");
    {
        // Re-call init to reload env
        let _guard = logger::init(&log_path2, LevelFilter::Info, true);

        info!("should_be_filtered");
        warn!("should_be_logged");

        log::logger().flush();
    }

    let content2 = fs::read_to_string(log_path2).unwrap();
    assert!(!content2.contains("should_be_filtered"));
    assert!(content2.contains("should_be_logged"));

    // Clean up env
    std::env::remove_var("LYRIC_LOG");
}
