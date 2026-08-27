//! 横向跑马灯状态机（30 DIP/秒，与 vsync 解耦）

pub const SCROLL_SPEED_DIP_PER_SECOND: f32 = 30.0;

#[derive(Debug, Clone)]
pub struct ScrollState {
    pub offset_x: f32,
    pub text_width: f32,
    pub window_width: f32,
    pub needs_recompute: bool,
    last_text: String,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollState {
    pub fn new() -> Self {
        Self {
            offset_x: 0.0,
            text_width: 0.0,
            window_width: 0.0,
            needs_recompute: true,
            last_text: String::new(),
        }
    }

    pub fn should_scroll(text_width: f32, window_width: f32) -> bool {
        text_width > window_width
    }

    /// 推进一帧。调用方须先写入最新 `text_width`（由 renderer 度量）。
    ///
    /// 文本变化时 `offset_x` 重置为 `window_width`（若仍需滚动）。
    /// 窗口变窄/变宽：仍超宽则保留 offset，否则归零。
    pub fn update(&mut self, dt: f32, current_text: &str, window_width: f32) {
        let prev_width = self.window_width;
        let width_changed = prev_width > 0.0 && (prev_width - window_width).abs() > f32::EPSILON;
        self.window_width = window_width;

        if current_text != self.last_text {
            self.last_text = current_text.to_owned();
            self.on_text_changed(self.text_width, window_width);
        } else if width_changed && !Self::should_scroll(self.text_width, window_width) {
            self.offset_x = 0.0;
        }

        if Self::should_scroll(self.text_width, window_width) {
            self.offset_x -= SCROLL_SPEED_DIP_PER_SECOND * dt.max(0.0);
            self.offset_x =
                Self::wrap_offset_if_needed(self.offset_x, self.text_width, window_width);
        } else {
            self.offset_x = 0.0;
        }
    }

    pub fn on_text_changed(&mut self, new_text_width: f32, window_width: f32) {
        self.text_width = new_text_width;
        self.window_width = window_width;
        if Self::should_scroll(new_text_width, window_width) {
            self.offset_x = window_width;
        } else {
            self.offset_x = 0.0;
        }
        self.needs_recompute = false;
    }

    pub fn reset_for_font_change(&mut self, new_text_width: f32, window_width: f32) {
        self.text_width = new_text_width;
        self.window_width = window_width;
        if Self::should_scroll(new_text_width, window_width) {
            self.offset_x = window_width;
        } else {
            self.offset_x = 0.0;
        }
    }

    pub fn wrap_offset_if_needed(offset_x: f32, text_width: f32, window_width: f32) -> f32 {
        if offset_x <= -text_width {
            window_width
        } else {
            offset_x
        }
    }
}
