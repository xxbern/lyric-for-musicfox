//! 设置页字段校验（req.md §10.4）

use crate::config::Config;
use crate::window::FALLBACK_FONT_FAMILY;

pub fn validate_width(v: u32) -> Result<(), String> {
    if (100..=4000).contains(&v) {
        Ok(())
    } else {
        Err("宽度须在 100..=4000 DIP".into())
    }
}

pub fn validate_height(v: u32) -> Result<(), String> {
    if (20..=500).contains(&v) {
        Ok(())
    } else {
        Err("高度须在 20..=500 DIP".into())
    }
}

pub fn validate_font_size(v: f32) -> Result<(), String> {
    if v.is_finite() && (6.0..=200.0).contains(&v) {
        Ok(())
    } else {
        Err("字号须为 6.0..=200.0 的有限值".into())
    }
}

pub fn validate_outline_width(v: u32) -> Result<(), String> {
    if (0..=16).contains(&v) {
        Ok(())
    } else {
        Err("描边宽度须在 0..=16 DIP".into())
    }
}

pub fn validate_color(s: &str) -> Result<(), String> {
    if is_rrggbb(s) {
        Ok(())
    } else {
        Err("颜色须为 #RRGGBB".into())
    }
}

pub fn is_rrggbb(s: &str) -> bool {
    let s = s.trim();
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn normalize_color(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

pub fn validate_font_family(name: &str) -> Result<(), String> {
    if crate::services::font::font_family_certainly_missing(name) {
        Err(format!(
            "字体 `{name}` 未安装；将回退 {FALLBACK_FONT_FAMILY}"
        ))
    } else {
        Ok(())
    }
}

pub fn validate_musicfox_path(path_str: &str) -> Result<(), String> {
    let s = path_str.trim();
    if s.is_empty() {
        return Err("go-musicfox 路径不能为空".into());
    }
    #[cfg(windows)]
    {
        let p = std::path::Path::new(s);
        if !p.exists() {
            return Err("go-musicfox 可执行文件不存在".into());
        }
    }
    #[cfg(not(windows))]
    {
        let p = std::path::Path::new(s);
        if !s.starts_with("C:") && !s.starts_with("c:") && !p.exists() {
            return Err("go-musicfox 可执行文件不存在".into());
        }
    }
    Ok(())
}

pub fn validate_all(config: &Config) -> Vec<(String, String)> {
    let mut errors = Vec::new();
    if let Err(e) = validate_width(config.window.width) {
        errors.push(("width".into(), e));
    }
    if let Err(e) = validate_height(config.window.height) {
        errors.push(("height".into(), e));
    }
    if let Err(e) = validate_font_size(config.lyric_style.font_size) {
        errors.push(("font_size".into(), e));
    }
    if let Err(e) = validate_outline_width(config.lyric_style.font_outline_width) {
        errors.push(("font_outline_width".into(), e));
    }
    if let Err(e) = validate_color(&config.lyric_style.font_color) {
        errors.push(("font_color".into(), e));
    }
    if let Some(c) = config.lyric_style.font_outline_color.as_deref() {
        if let Err(e) = validate_color(c) {
            errors.push(("font_outline_color".into(), e));
        }
    }
    if let Err(e) = validate_font_family(&config.lyric_style.font_family) {
        errors.push(("font_family".into(), e));
    }
    if let Err(e) = validate_musicfox_path(&config.wt.musicfox_path) {
        errors.push(("musicfox_path".into(), e));
    }
    errors
}

pub fn normalize_colors_in_place(config: &mut Config) {
    if is_rrggbb(&config.lyric_style.font_color) {
        config.lyric_style.font_color = normalize_color(&config.lyric_style.font_color);
    }
    if let Some(c) = config.lyric_style.font_outline_color.as_mut() {
        if is_rrggbb(c) {
            *c = normalize_color(c);
        }
    }
}
