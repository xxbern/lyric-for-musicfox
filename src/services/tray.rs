//! 托盘控制服务：托盘构建与闪烁状态管理。
//!
//! ⚠️ 临时保留（dead code）：stage-3 评审决议保留以备下一阶段重构；
//! 目前未被任何调用点引用。所有托盘逻辑都走 `crate::tray`（平台 trait 后端）。
//! 下次重构阶段统一清理。

use std::sync::Arc;
use crate::context::AppContext;
use crate::tray::{TrayCmd, TrayHandle};

#[allow(dead_code)]
#[derive(Clone)]
pub struct TrayService {
    ctx: Arc<AppContext>,
}

impl TrayService {
    #[allow(dead_code)]
    pub fn new(ctx: Arc<AppContext>) -> Self {
        Self { ctx }
    }

    #[allow(dead_code)]
    pub fn init_tray(
        &self,
        event_bus: crate::event_bus::EventBus,
    ) -> (Option<TrayHandle>, crossbeam_channel::Receiver<TrayCmd>) {
        match crate::platform::current().init(event_bus) {
            Ok(res) => res,
            Err(e) => {
                log::error!("Failed to initialize tray: {e}");
                let (_tx, rx) = crossbeam_channel::unbounded();
                (None, rx)
            }
        }
    }

    #[allow(dead_code)]
    pub fn destroy_tray(&self) {}

    #[allow(dead_code)]
    pub fn update_flash(&self, handle: &Option<TrayHandle>, flash: bool) {
        crate::platform::current().update_flash(handle, flash);
    }
}
