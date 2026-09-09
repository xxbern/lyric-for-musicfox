use std::sync::Arc;
use crate::context::AppContext;
use crate::error::AppError;
use crate::tray::{TrayCmd, TrayHandle};
use crate::services::monitor::MonitorLayout;

pub trait PlatformSessionId {
    fn session_id(&self) -> u32;
}

pub trait PlatformInstance {
    fn acquire_main_mutex(&self) -> Result<Option<crate::platform::MutexHandle>, AppError>;
    fn acquire_settings_mutex(&self) -> Result<Option<crate::platform::MutexHandle>, AppError>;
    fn release_mutex(&self, h: Option<crate::platform::MutexHandle>);
    fn probe_port(&self, port: u16) -> Result<(), AppError>;
}

pub trait PlatformTray {
    fn init(&self, event_bus: crate::event_bus::EventBus) -> Result<(Option<TrayHandle>, crossbeam_channel::Receiver<TrayCmd>), String>;
    fn update_flash(&self, handle: &Option<TrayHandle>, flash: bool);
}

pub trait PlatformPipeServer {
    fn start_reload(&self, ctx: Arc<AppContext>, config_path: std::path::PathBuf) -> Result<(), String>;
    fn start_pos(&self, ctx: Arc<AppContext>) -> Result<(), String>;
    fn start_presence(&self) -> Result<(), String>;
}

pub trait PlatformPipeClient {
    fn send(&self, base: &str, payload: &[u8], timeout_ms: u32) -> Result<Vec<u8>, String>;
    fn try_activate(&self, pid: u32) -> Result<(), String>;
}

pub trait PlatformWindowStyle {
    /// 从 raw-window-handle 提取原生窗口句柄。
    fn from_raw(&self, raw: raw_window_handle::RawWindowHandle) -> Option<crate::platform::WindowHandle>;
    fn apply_locked_style(&self, h: crate::platform::WindowHandle, locked: bool);
    fn apply_outer_position(&self, h: crate::platform::WindowHandle, pos: (i32, i32));
    fn refresh_stay_on_top(&self, h: crate::platform::WindowHandle);
    fn window_outer_position(&self, h: crate::platform::WindowHandle) -> Option<(i32, i32)>;
    fn cursor_position(&self) -> Option<(i32, i32)>;
    fn install_display_change_hook(&self, h: crate::platform::WindowHandle, ctx: Arc<AppContext>);
}

pub trait PlatformShell {
    fn reveal_in_file_manager(&self, path: &std::path::Path);
}

pub trait PlatformMonitor {
    fn enumerate(&self) -> MonitorLayout;
    fn primary_pixels_per_point(&self, layout: &MonitorLayout) -> f32;
}

pub trait PlatformWt {
    fn find_wt_window(&self, title: &str) -> Option<crate::platform::WindowHandle>;
    fn launch_wt(&self, app_dir: &str, title: &str, musicfox_path: &str) -> Result<(), String>;
    fn apply_wt_hosted_style(&self, h: crate::platform::WindowHandle);
    fn set_wt_alpha(&self, h: crate::platform::WindowHandle, alpha: u8);
    fn is_wt_hidden(&self, h: crate::platform::WindowHandle) -> bool;
    fn activate_wt(&self, h: crate::platform::WindowHandle);
    fn install_wt_minimize_hook(&self, title: String);
    /// 仅卸载最小化事件钩子，不关闭 WT 窗口。
    /// 用于“保存配置重启 lyric 但保留 go-musicfox ”场景（pipe::reload::notify_reload）。
    fn uninstall_wt_minimize_hook(&self, _title: &str) {}
    fn shutdown_wt(&self, title: &str);
}

pub trait Platform:
    PlatformSessionId
    + PlatformInstance
    + PlatformTray
    + PlatformPipeServer
    + PlatformPipeClient
    + PlatformWindowStyle
    + PlatformShell
    + PlatformMonitor
    + PlatformWt
{}
