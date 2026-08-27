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

    pub fn height(&self) -> i32 {
        self.bottom - self.top
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

fn window_intersects_monitor(
    pos: (i32, i32),
    window_size_physical: (i32, i32),
    mon: &MonitorRect,
) -> bool {
    let right = pos.0.saturating_add(window_size_physical.0);
    let bottom = pos.1.saturating_add(window_size_physical.1);
    pos.0 < mon.right && right > mon.left && pos.1 < mon.bottom && bottom > mon.top
}

/// 启动期裁剪：窗口与任一已枚举显示器相交则原样保留（副屏仍在 → 位置保留）。
/// 完全不可见时钳到主屏整屏（req.md §14 + Reviewer D1）。
/// 基准是 rcMonitor（不是 rcWork）。左/上边界不强制到 `0` / `mon.left`。
pub fn clamp_to_primary_monitor(
    pos: (i32, i32),
    layout: &MonitorLayout,
    window_size_physical: (i32, i32),
) -> (i32, i32) {
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
    let x = pos.0.max(mon.left).min(mon.right - win_w);
    let y = pos.1.max(mon.top).min(mon.bottom - win_h);
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
