//! 配置服务：磁盘读写与热重载广播。

use std::sync::Arc;
use crate::context::AppContext;

#[derive(Clone)]
pub struct ConfigService {
    ctx: Arc<AppContext>,
}

impl ConfigService {
    pub fn new(ctx: Arc<AppContext>) -> Self {
        Self { ctx }
    }

    pub fn load_or_default() -> crate::AppResult<crate::Config> {
        let path = crate::path::config_path()?;
        crate::load_config(&path)
    }

    pub fn save(&self, cfg: &crate::Config) -> crate::AppResult<()> {
        let path = crate::path::config_path()?;
        crate::save_config(cfg, &path).map(|_| ())
    }

    /// 从磁盘重载并广播 ConfigReloaded；成功返回新配置。
    pub fn reload_from_disk(&self) -> Option<crate::Config> {
        let path = crate::path::config_path().ok()?;
        let cfg = crate::load_config(&path).ok()?;
        self.ctx.update_config(cfg.clone());
        Some(cfg)
    }
}
