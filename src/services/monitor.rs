//! 静态显示器枚举 + 虚拟桌面绝对坐标裁剪。
//! P2 不实现动态插拔（WM_DISPLAYCHANGE → P4）。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl MonitorRect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorLayout {
    /// 所有显示器物理像素矩形（跨屏连续虚拟桌面坐标）
    pub monitors: Vec<MonitorRect>,
    /// 主显示器索引
    pub primary: usize,
    /// 主显示器**整屏**矩形（rcMonitor，非 rcWork）
    pub primary_monitor: MonitorRect,
}

const FALLBACK_MONITOR: MonitorRect = MonitorRect {
    left: 0,
    top: 0,
    right: 1920,
    bottom: 1080,
};

fn fallback_layout() -> MonitorLayout {
    MonitorLayout {
        monitors: vec![FALLBACK_MONITOR],
        primary: 0,
        primary_monitor: FALLBACK_MONITOR,
    }
}

/// 静态枚举（委托平台后端）；失败时降级为 1920×1080 主屏占位。
pub fn enumerate() -> MonitorLayout {
    crate::platform::current().enumerate()
}

pub fn primary_pixels_per_point(layout: &MonitorLayout) -> f32 {
    crate::platform::current().primary_pixels_per_point(layout)
}

pub(crate) fn fallback() -> MonitorLayout {
    fallback_layout()
}

/// 判断当前显示器布局是否为 Windows 熄屏、睡眠断联或驱动重置产生的瞬态虚拟小屏。
/// 典型特征：仅有单一显示器且分辨率 <= 1024×768（如 1024x768、800x600、640x480）。
pub fn is_transient_layout(layout: &MonitorLayout) -> bool {
    if layout.monitors.is_empty() {
        return true;
    }
    if layout.monitors.len() == 1 {
        let mon = &layout.monitors[0];
        let w = mon.width();
        let h = mon.bottom.saturating_sub(mon.top);
        w.min(h) <= 768 && w.max(h) <= 1024
    } else {
        false
    }
}

pub fn window_intersects_monitor(
    pos: (i32, i32),
    window_size_physical: (i32, i32),
    mon: &MonitorRect,
) -> bool {
    let right = pos.0.saturating_add(window_size_physical.0);
    let bottom = pos.1.saturating_add(window_size_physical.1);
    pos.0 < mon.right && right > mon.left && pos.1 < mon.bottom && bottom > mon.top
}

/// 显示器坐标裁剪：
/// 1. 若当前处于熄屏/休眠引发的瞬态虚拟小屏，跳过破坏性裁剪，原样保留意图坐标。
/// 2. 窗口与任一已枚举显示器相交（含部分边缘负坐标微溢出）则原样保留。
/// 3. 完全不可见时钳到主屏整屏（正交保留仍相交维度的坐标，如保留贴顶负坐标）。
pub fn clamp_to_primary_monitor(
    pos: (i32, i32),
    layout: &MonitorLayout,
    window_size_physical: (i32, i32),
) -> (i32, i32) {
    if is_transient_layout(layout) {
        log::debug!("skipping clamp: transient display layout detected ({:?})", layout.monitors);
        return pos;
    }

    let visible = if layout.monitors.is_empty() {
        window_intersects_monitor(pos, window_size_physical, &layout.primary_monitor)
    } else {
        layout
            .monitors
            .iter()
            .any(|mon| window_intersects_monitor(pos, window_size_physical, mon))
    };
    if visible {
        return pos;
    }

    let mon = &layout.primary_monitor;
    let (win_w, win_h) = window_size_physical;
    let max_x = mon.right.saturating_sub(win_w).max(mon.left);
    let x = if pos.0 < mon.right && pos.0.saturating_add(win_w) > mon.left {
        pos.0
    } else {
        pos.0.clamp(mon.left, max_x)
    };
    let max_y = mon.bottom.saturating_sub(win_h).max(mon.top);
    let y = if pos.1 < mon.bottom && pos.1.saturating_add(win_h) > mon.top {
        pos.1
    } else {
        pos.1.clamp(mon.top, max_y)
    };
    (x, y)
}

/// 启动期 None → 主显示器整屏水平居中 + 顶部偏下 100 DIP 转物理像素。
pub fn center_on_primary_monitor(
    layout: &MonitorLayout,
    window_size_physical: (i32, i32),
    pixels_per_point_primary: f32,
) -> (i32, i32) {
    let mon = &layout.primary_monitor;
    let (win_w, _win_h) = window_size_physical;
    let x = mon.left + (mon.width() - win_w) / 2;
    let y = mon.top + (100.0 * pixels_per_point_primary).round() as i32;
    (x, y)
}


/// 显示器服务：枚举与坐标裁剪。
#[derive(Clone, Copy)]
pub struct MonitorService;

impl MonitorService {
    pub fn new() -> Self {
        Self
    }

    pub fn enumerate_screens(&self) -> MonitorLayout {
        enumerate()
    }

    pub fn is_transient_layout(&self, layout: &MonitorLayout) -> bool {
        is_transient_layout(layout)
    }

    pub fn clamp_position(
        &self,
        pos: (i32, i32),
        layout: &MonitorLayout,
        window_size_physical: (i32, i32),
    ) -> (i32, i32) {
        clamp_to_primary_monitor(pos, layout, window_size_physical)
    }

    pub fn primary_pixels_per_point(&self, layout: &MonitorLayout) -> f32 {
        primary_pixels_per_point(layout)
    }
}

impl Default for MonitorService {
    fn default() -> Self {
        Self::new()
    }
}
