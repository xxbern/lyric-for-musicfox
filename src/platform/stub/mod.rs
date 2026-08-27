use std::sync::Arc;
use crate::context::AppContext;
use crate::error::AppError;
use crate::tray::{TrayCmd, TrayHandle};
use super::r#trait::*;

pub struct StubBackend;

impl Platform for StubBackend {}

impl PlatformSessionId for StubBackend {
    fn session_id(&self) -> u32 {
        0
    }
}

impl PlatformInstance for StubBackend {
    fn acquire_main_mutex(&self) -> Result<Option<super::MutexHandle>, AppError> {
        Ok(None)
    }

    fn acquire_settings_mutex(&self) -> Result<Option<super::MutexHandle>, AppError> {
        Ok(None)
    }

    fn release_mutex(&self, _h: Option<super::MutexHandle>) {}

    fn probe_port(&self, _port: u16) -> Result<(), AppError> {
        Ok(())
    }
}

impl PlatformTray for StubBackend {
    fn init(&self, _event_bus: crate::event_bus::EventBus) -> Result<(Option<TrayHandle>, crossbeam_channel::Receiver<TrayCmd>), String> {
        let (_tx, rx) = crossbeam_channel::unbounded();
        Ok((None, rx))
    }

    fn update_flash(&self, _handle: &Option<TrayHandle>, _flash: bool) {}
}

impl PlatformPipeServer for StubBackend {
    fn start_reload(&self, _ctx: Arc<AppContext>, _config_path: std::path::PathBuf) -> Result<(), String> {
        Ok(())
    }

    fn start_pos(&self, _ctx: Arc<AppContext>) -> Result<(), String> {
        Ok(())
    }

    fn start_presence(&self) -> Result<(), String> {
        Ok(())
    }
}

impl PlatformPipeClient for StubBackend {
    fn send(&self, _base: &str, _payload: &[u8], _timeout_ms: u32) -> Result<Vec<u8>, String> {
        Err("Pipe client not supported on this platform".to_string())
    }

    fn try_activate(&self, _pid: u32) -> Result<(), String> {
        Err("Pipe client try_activate not supported on this platform".to_string())
    }
}

impl PlatformWindowStyle for StubBackend {
    fn from_raw(&self, _raw: raw_window_handle::RawWindowHandle) -> Option<super::WindowHandle> {
        None
    }

    fn apply_locked_style(&self, _h: super::WindowHandle, _locked: bool) {}

    fn apply_outer_position(&self, _h: super::WindowHandle, _pos: (i32, i32)) {}

    fn refresh_stay_on_top(&self, _h: super::WindowHandle) {}

    fn window_outer_position(&self, _h: super::WindowHandle) -> Option<(i32, i32)> {
        None
    }

    fn cursor_position(&self) -> Option<(i32, i32)> {
        None
    }

    fn has_transparent_style(&self, _h: super::WindowHandle) -> bool {
        false
    }

    fn install_display_change_hook(&self, _h: super::WindowHandle, _ctx: Arc<AppContext>) {}
}

impl PlatformShell for StubBackend {
    fn reveal_in_file_manager(&self, path: &std::path::Path) {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

impl PlatformMonitor for StubBackend {
    fn enumerate(&self) -> crate::services::monitor::MonitorLayout {
        crate::services::monitor::fallback()
    }

    fn primary_pixels_per_point(&self, _layout: &crate::services::monitor::MonitorLayout) -> f32 {
        1.0
    }
}

impl PlatformWt for StubBackend {
    fn find_wt_window(&self, _title: &str) -> Option<super::WindowHandle> {
        None
    }

    fn launch_wt(&self, _app_dir: &str, _title: &str, _musicfox_path: &str) -> Result<(), String> {
        Ok(())
    }

    fn apply_wt_hosted_style(&self, _h: super::WindowHandle) {}

    fn set_wt_alpha(&self, _h: super::WindowHandle, _alpha: u8) {}

    fn is_wt_hidden(&self, _h: super::WindowHandle) -> bool {
        false
    }

    fn activate_wt(&self, _h: super::WindowHandle) {}

    fn install_wt_minimize_hook(&self, _title: String) {}

    fn shutdown_wt(&self, _title: &str) {}
}

pub fn restart_lyric_app() {
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
    });
}
