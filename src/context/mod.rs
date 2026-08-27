use crate::event_bus::EventBus;
use crate::lyric::state::LyricState;
use crate::Config;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

pub struct Signals {
    pub needs_reload: Arc<AtomicBool>,
    pub is_dragging: Arc<AtomicBool>,
    pub display_changed: Arc<AtomicBool>,
    pub dpi_recheck_pending: Arc<AtomicBool>,
}

impl Default for Signals {
    fn default() -> Self {
        Self {
            needs_reload: Arc::new(AtomicBool::new(false)),
            is_dragging: Arc::new(AtomicBool::new(false)),
            display_changed: Arc::new(AtomicBool::new(false)),
            dpi_recheck_pending: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct AppContext {
    pub config: RwLock<Config>,
    pub state: RwLock<LyricState>,
    pub signals: Signals,
    pub event_bus: EventBus,
}

impl AppContext {
    pub fn new(config: Config, state: LyricState, event_bus: EventBus) -> Self {
        Self {
            config: RwLock::new(config),
            state: RwLock::new(state),
            signals: Signals::default(),
            event_bus,
        }
    }
    pub fn get_config(&self) -> Config {
        self.config
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn update_config(&self, new_cfg: Config) {
        {
            let mut lock = self.config.write().unwrap_or_else(|p| p.into_inner());
            *lock = new_cfg;
        }
        self.event_bus
            .emit(crate::event_bus::AppEvent::ConfigReloaded(Box::new(
                self.get_config(),
            )));
    }
}
