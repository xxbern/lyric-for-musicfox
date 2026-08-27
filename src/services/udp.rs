//! UDP 歌词监听服务：接收线程生命周期管理。

use std::net::UdpSocket;
use std::sync::Arc;
use crate::context::AppContext;

#[derive(Clone)]
pub struct LyricUdpService {
    #[allow(dead_code)]
    ctx: Arc<AppContext>,
}

impl LyricUdpService {
    pub fn new(ctx: Arc<AppContext>) -> Self {
        Self { ctx }
    }

    /// 启动后台接收线程。
    pub fn start_listening(&self, socket: UdpSocket) {
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            crate::lyric::udp::recv_loop(socket, ctx);
        });
    }

    pub fn stop_listening(&self) {}
}
