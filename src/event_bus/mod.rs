use crate::Config;
use crossbeam_channel::{bounded, Receiver, Sender};

#[derive(Debug, Clone)]
pub enum AppEvent {
    RequestRepaint,
    ConfigReloaded(Box<Config>),
    TrayCmd(crate::tray::TrayCmd),
    LyricStateChanged,
}

#[derive(Clone)]
pub struct EventBus {
    sender: Sender<AppEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> (Self, Receiver<AppEvent>) {
        let (tx, rx) = bounded(capacity);
        (Self { sender: tx }, rx)
    }
    pub fn emit(&self, ev: AppEvent) {
        if let Err(e) = self.sender.try_send(ev) {
            log::warn!("event bus drop: {:?}", e);
        }
    }
    pub fn sender(&self) -> Sender<AppEvent> {
        self.sender.clone()
    }
}
