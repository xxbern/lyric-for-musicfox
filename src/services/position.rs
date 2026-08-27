//! 窗口位置服务：共享状态坐标存取与拖拽落点应用。

use std::sync::Arc;
use crate::context::AppContext;
use crate::platform::WindowHandle;

#[derive(Clone)]
pub struct PositionService {
    ctx: Arc<AppContext>,
}

impl PositionService {
    pub fn new(ctx: Arc<AppContext>) -> Self {
        Self { ctx }
    }

    pub fn get_current_pos(&self) -> (i32, i32) {
        let s = self.ctx.state.read().unwrap_or_else(|p| p.into_inner());
        (s.pos_x, s.pos_y)
    }

    /// 将坐标写入共享状态；返回是否发生变化。
    pub fn set_current_pos(&self, pos: (i32, i32)) -> bool {
        let mut s = self.ctx.state.write().unwrap_or_else(|p| p.into_inner());
        if (s.pos_x, s.pos_y) == pos {
            return false;
        }
        s.pos_x = pos.0;
        s.pos_y = pos.1;
        true
    }

    /// 应用窗口外框位置到平台窗口，并同步共享状态。
    pub fn apply_window_pos(&self, hwnd: WindowHandle, pos: (i32, i32)) {
        crate::platform::current().apply_outer_position(hwnd, pos);
        self.set_current_pos(pos);
    }
}
