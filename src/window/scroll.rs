//! 歌词文本排版几何计算：静态居中与溢出边界判定。

/// 计算静态排版下的 X 轴基准坐标：
/// - 未超宽：水平居中 `(window_width - text_width) / 2.0`
/// - 超宽：靠左对齐 `0.0`（右侧超出部分由 Slint 视口容器裁剪截断隐藏）
pub fn compute_base_x(text_width: f32, window_width: f32) -> f32 {
    if is_overflowing(text_width, window_width) {
        0.0
    } else {
        ((window_width - text_width) / 2.0).max(0.0)
    }
}

/// 判定文本是否超出窗口可视宽度。
pub fn is_overflowing(text_width: f32, window_width: f32) -> bool {
    text_width > window_width
}

/// 静态排版状态记录
#[derive(Debug, Clone, Default)]
pub struct LayoutState {
    pub text_width: f32,
    pub window_width: f32,
    pub needs_recompute: bool,
}

impl LayoutState {
    pub fn new() -> Self {
        Self {
            text_width: 0.0,
            window_width: 0.0,
            needs_recompute: true,
        }
    }

    pub fn update(&mut self, text_width: f32, window_width: f32) -> f32 {
        self.text_width = text_width;
        self.window_width = window_width;
        self.needs_recompute = false;
        compute_base_x(text_width, window_width)
    }
}

/// 兼容别名
pub type ScrollState = LayoutState;
