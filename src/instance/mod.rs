//! 跨平台单实例 + 端口探测 shim
pub use crate::platform::MutexHandle;

use crate::error::AppError;

pub fn acquire_main_mutex() -> Result<Option<MutexHandle>, AppError> {
    crate::platform::current().acquire_main_mutex()
}

pub fn acquire_settings_mutex() -> Result<Option<MutexHandle>, AppError> {
    crate::platform::current().acquire_settings_mutex()
}

pub fn release_main_mutex(handle: Option<MutexHandle>) {
    crate::platform::current().release_mutex(handle);
}

pub fn release_settings_mutex(handle: Option<MutexHandle>) {
    crate::platform::current().release_mutex(handle);
}
