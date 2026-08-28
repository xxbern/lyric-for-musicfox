//! Windows Mutex 单实例测试
//! 仅在 Windows target 下编译运行（Linux 上 instance::acquire_main_mutex 是 stub）
#![cfg(windows)]

use lyric_for_musicfox::instance::{
    acquire_main_mutex, acquire_settings_mutex, release_main_mutex, release_settings_mutex,
};
use lyric_for_musicfox::AppError;

use std::sync::Mutex;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_mutex_acquire_twice_returns_another_instance() {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());

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
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());

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
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());

    let main_handle = acquire_main_mutex().expect("main mutex first");
    let settings_handle = acquire_settings_mutex().expect("settings mutex first");
    assert!(main_handle.is_some());
    assert!(settings_handle.is_some());
    release_main_mutex(main_handle);
    release_settings_mutex(settings_handle);
}
