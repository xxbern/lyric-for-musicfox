pub mod r#trait;
#[cfg(windows)]
pub mod windows;
#[cfg(not(windows))]
pub mod stub;

pub use r#trait::*;

#[cfg(windows)]
pub type MutexHandle = ::windows::Win32::Foundation::HANDLE;
#[cfg(not(windows))]
pub type MutexHandle = ();

#[cfg(windows)]
pub type WindowHandle = ::windows::Win32::Foundation::HWND;
#[cfg(not(windows))]
pub type WindowHandle = ();

pub fn current() -> &'static dyn Platform {
    #[cfg(windows)]
    {
        &windows::WindowsBackend
    }
    #[cfg(not(windows))]
    {
        &stub::StubBackend
    }
}
