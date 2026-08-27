//! 渲染输入变更追踪：样式/文本键变化时通知滚动状态机重置。

use crate::config::LyricStyleConfig;

#[derive(Default)]
pub struct RenderCache {
    text: String,
    font_family: String,
    font_size: f32,
    bold: bool,
    italic: bool,
    playing: bool,
    is_placeholder: bool,
}

impl RenderCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 键变化 → 需要重置滚动（reset_for_font_change）。
    pub fn update_key(
        &mut self,
        new_text: &str,
        style: &LyricStyleConfig,
        font_family: &str,
        is_placeholder: bool,
        playing: bool,
    ) -> bool {
        let changed = self.text != new_text
            || self.font_family != font_family
            || (self.font_size - style.font_size).abs() > f32::EPSILON
            || self.bold != style.font_bold
            || self.italic != style.font_italic
            || self.playing != playing
            || self.is_placeholder != is_placeholder;
        if changed {
            self.text = new_text.to_string();
            self.font_family = font_family.to_string();
            self.font_size = style.font_size;
            self.bold = style.font_bold;
            self.italic = style.font_italic;
            self.playing = playing;
            self.is_placeholder = is_placeholder;
        }
        changed
    }

    /// 最近一次键更新时的 playing 快照（用于滚动/重绘判定）。
    pub fn playing_snapshot(&self) -> bool {
        self.playing
    }
}
