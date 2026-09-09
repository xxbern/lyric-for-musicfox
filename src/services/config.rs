//! 配置服务：磁盘读写接口。

use std::sync::Arc;
use crate::context::AppContext;

#[derive(Clone, Default)]
pub struct ConfigService;

impl ConfigService {
    pub fn new(_ctx: Arc<AppContext>) -> Self {
        Self
    }

    pub fn load_or_default() -> crate::AppResult<crate::Config> {
        let path = crate::path::config_path()?;
        crate::load_config(&path)
    }
}
