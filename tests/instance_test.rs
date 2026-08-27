//! Windows Mutex 单实例测试
//! 仅在 Windows target 下编译运行（Linux 上 instance::acquire_main_mutex 是 stub）
#![cfg(windows)]

use lyric_for_musicfox::instance::{
    acquire_main_mutex, acquire_settings_mutex, release_main_mutex, release_settings_mutex,
};
use lyric_for_musicfox::AppError;

fn run_mutex_test(name: &str, test: impl Fn() + std::panic::UnwindSafe) {
    let pid = std::process::id();
    let test_name = format!("{} (pid {})", name, pid);
    let result = std::panic::catch_unwind(test);
    if result.is_err() {
        eprintln!("FAIL: {}", test_name);
    }
    assert!(result.is_ok(), "Test panicked: {}", test_name);
}

#[test]
fn test_mutex_acquire_twice_returns_another_instance() {
    // 第一次获取：第一个实例成功
    let first = acquire_main_mutex().expect("first acquire should succeed");
    assert!(first.is_some());

    // 第二次获取：应报告另一个实例
    let second = acquire_main_mutex();
    match second {
        Err(AppError::AnotherInstance) => { /* expected */ }
        other => panic!("expected AnotherInstance, got: {other:?}"),
    }

    // 清理
    release_main_mutex(first);
}

#[test]
fn test_settings_mutex_acquire_twice_returns_another_instance() {
    let first = acquire_settings_mutex().expect("first settings acquire should succeed");
    assert!(first.is_some());

    let second = acquire_settings_mutex();
    match second {
        Err(AppError::AnotherInstance) => { /* expected */ }
        other => panic!("expected AnotherInstance, got: {other:?}"),
    }

    release_settings_mutex(first);
}

#[test]
fn test_main_and_settings_mutex_are_independent() {
    let main_handle = acquire_main_mutex().expect("main mutex first");
    let settings_handle = acquire_settings_mutex().expect("settings mutex first");
    assert!(main_handle.is_some());
    assert!(settings_handle.is_some());
    release_main_mutex(main_handle);
    release_settings_mutex(settings_handle);
}
