//! lyric-for-musicfox 库入口（P2：GUI + 设置 + 拖动/锁定/副屏）

// Slint 生成的组件（LyricWindow / SettingsWindow）统一在此导出，避免多处 include 重复定义
slint::include_modules!();

pub mod cli;
pub mod command;
pub mod config;
pub mod context;
pub mod diag;
pub mod error;
pub mod event_bus;
pub mod instance;
pub mod logger;
pub mod lyric;
pub mod path;
pub mod pipe;
pub mod r#settings;
pub mod tray;
pub mod window;
pub mod platform;
pub mod protocol;
pub mod services;

// Re-exports
pub use config::load::load as load_config;
pub use config::save::save as save_config;
pub use config::{Config, LyricStyleConfig, SystemConfig, WindowConfig};
pub use error::{AppError, AppResult};

pub use platform::MutexHandle;

pub fn probe_port(port: u16) -> Result<(), AppError> {
    platform::current().probe_port(port)
}
