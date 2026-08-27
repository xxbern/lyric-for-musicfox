//! 业务服务聚合：各服务仅持有 `Arc<AppContext>`，彼此解耦。

pub mod config;
pub mod font;
pub mod monitor;
pub mod position;
pub mod style;
pub mod tray;
pub mod udp;
pub mod wt;

use std::sync::Arc;
use crate::context::AppContext;

#[derive(Clone)]
pub struct ServiceHandles {
    pub config: config::ConfigService,
    pub font: font::FontService,
    pub monitor: monitor::MonitorService,
    pub position: position::PositionService,
    pub style: style::WindowStyleService,
    pub tray: tray::TrayService,
    pub udp: udp::LyricUdpService,
    pub wt: wt::WtService,
}

impl ServiceHandles {
    pub fn new(ctx: Arc<AppContext>) -> Self {
        Self {
            config: config::ConfigService::new(ctx.clone()),
            font: font::FontService::new(ctx.clone()),
            monitor: monitor::MonitorService::new(),
            position: position::PositionService::new(ctx.clone()),
            style: style::WindowStyleService::new(ctx.clone()),
            tray: tray::TrayService::new(ctx.clone()),
            udp: udp::LyricUdpService::new(ctx.clone()),
            wt: wt::WtService::new(ctx),
        }
    }
}
