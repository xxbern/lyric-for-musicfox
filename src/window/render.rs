//! 颜色解析与 alpha 应用（渲染本身已声明式迁入 ui/lyric.slint）

use crate::config::LyricStyleConfig;
use crate::lyric::state::text_alpha;
use slint::Color;

pub fn parse_rrggbb(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();
    if s.len() != 7 || !s.starts_with('#') {
        return None;
    }
    let r = u8::from_str_radix(&s[1..3], 16).ok()?;
    let g = u8::from_str_radix(&s[3..5], 16).ok()?;
    let b = u8::from_str_radix(&s[5..7], 16).ok()?;
    Some((r, g, b))
}

pub fn color_with_alpha(s: &str, alpha: f32) -> Color {
    let (r, g, b) = parse_rrggbb(s).unwrap_or((255, 255, 255));
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::from_argb_encoded(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32)
}

/// 占位文本 / 当前歌词文本 + 对应透明度。
pub fn current_text_color(style: &LyricStyleConfig, is_placeholder: bool, playing: bool) -> Color {
    color_with_alpha(&style.font_color, text_alpha(is_placeholder, playing))
}
