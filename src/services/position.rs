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

    /// 获取意图坐标（用户配置或最后一次拖拽释放的期望位置）
    pub fn get_target_pos(&self) -> (i32, i32) {
        let s = self.ctx.state.read().unwrap_or_else(|p| p.into_inner());
        (s.target_pos_x, s.target_pos_y)
    }

    /// 更新意图坐标（仅在配置加载或拖拽完成时调用）
    pub fn set_target_pos(&self, pos: (i32, i32)) {
        let mut s = self.ctx.state.write().unwrap_or_else(|p| p.into_inner());
        s.target_pos_x = pos.0;
        s.target_pos_y = pos.1;
    }

    /// 同时更新意图坐标与当前坐标（如初始化或配置热重载）
    pub fn sync_all_pos(&self, pos: (i32, i32)) {
        let mut s = self.ctx.state.write().unwrap_or_else(|p| p.into_inner());
        s.target_pos_x = pos.0;
        s.target_pos_y = pos.1;
        s.pos_x = pos.0;
        s.pos_y = pos.1;
    }

    /// 将当前投影坐标写入共享状态；返回是否发生变化。
    pub fn set_current_pos(&self, pos: (i32, i32)) -> bool {
        let mut s = self.ctx.state.write().unwrap_or_else(|p| p.into_inner());
        if (s.pos_x, s.pos_y) == pos {
            return false;
        }
        s.pos_x = pos.0;
        s.pos_y = pos.1;
        true
    }

    /// 应用窗口外框位置到平台窗口，并同步当前投影坐标（不改变意图坐标）。
    pub fn apply_window_pos(&self, hwnd: WindowHandle, pos: (i32, i32)) {
        crate::platform::current().apply_outer_position(hwnd, pos);
        self.set_current_pos(pos);
    }

    /// 应用窗口外框位置，并同时确认为新的意图坐标（如拖拽释放完成）。
    pub fn apply_and_sync_target(&self, hwnd: WindowHandle, pos: (i32, i32)) {
        crate::platform::current().apply_outer_position(hwnd, pos);
        self.sync_all_pos(pos);
    }
}
