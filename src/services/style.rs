//! 外观服务：鼠标穿透锁定与前台 Stay-on-top 定时刷新。

use std::sync::Arc;
use crate::context::AppContext;
use crate::platform::WindowHandle;

#[derive(Clone, Default)]
pub struct WindowStyleService;

impl WindowStyleService {
    pub fn new(_ctx: Arc<AppContext>) -> Self {
        Self
    }

    /// 锁定 = 鼠标穿透（WS_EX_TRANSPARENT）；解锁恢复可点击。
    pub fn set_click_through(&self, hwnd: WindowHandle, locked: bool) {
        crate::platform::current().apply_locked_style(hwnd, locked);
    }

    pub fn enforce_always_on_top(&self, hwnd: WindowHandle) {
        crate::platform::current().refresh_stay_on_top(hwnd);
    }
}
