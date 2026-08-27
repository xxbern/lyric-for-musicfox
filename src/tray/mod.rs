#[cfg(windows)]
use std::rc::Rc;
#[cfg(windows)]
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCmd {
    OpenSettings,
    ReloadLyrics,
    ToggleWt,
    ShowContextMenu,
    Quit,
}

#[cfg(windows)]
pub struct TrayHandle {
    pub icon_normal: Rc<tray_icon::Icon>,
    pub icon_gray: Rc<tray_icon::Icon>,
    pub tray: Arc<Mutex<Option<tray_icon::TrayIcon>>>,
    pub flashing: Arc<Mutex<bool>>,
}

#[cfg(not(windows))]
pub struct TrayHandle;

pub fn init_tray(event_bus: crate::event_bus::EventBus) -> (Option<TrayHandle>, crossbeam_channel::Receiver<TrayCmd>) {
    match crate::platform::current().init(event_bus) {
        Ok(res) => res,
        Err(e) => {
            log::error!("Failed to initialize tray: {e}");
            let (_tx, rx) = crossbeam_channel::unbounded();
            (None, rx)
        }
    }
}

pub fn update_tray_flash(handle: &Option<TrayHandle>, flash: bool) {
    crate::platform::current().update_flash(handle, flash);
}
