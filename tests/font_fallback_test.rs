//! 字体回退测试（P4-R：无 fontdb，走平台定向查询）
//!
//! Oracle 计划：
//! - 不可能字体 → Microsoft YaHei
//! - 不修改 config

use lyric_for_musicfox::config::{Config, LyricStyleConfig};
use lyric_for_musicfox::window::{font_family_installed, resolve_runtime_font, FALLBACK_FONT_FAMILY};

#[test]
fn missing_family_falls_back_to_microsoft_yahei() {
    let resolved = resolve_runtime_font("__lyric_for_musicfox_definitely_missing_font__");
    if cfg!(windows) {
        // Windows 上 GDI 可确定性判定缺失 → 回退
        assert_eq!(resolved, FALLBACK_FONT_FAMILY);
    } else {
        // 非 Windows 平台无法验证（None 语义）→ 保持请求值不误伤
        assert_eq!(resolved, "__lyric_for_musicfox_definitely_missing_font__");
    }
}

#[test]
fn fallback_does_not_mutate_caller_config() {
    let original = "BadFontName";
    let config = Config {
        lyric_style: LyricStyleConfig {
            font_family: original.to_string(),
            ..LyricStyleConfig::default()
        },
        ..Config::default()
    };
    let _ = resolve_runtime_font(&config.lyric_style.font_family);
    assert_eq!(config.lyric_style.font_family, original);
}

#[test]
fn membership_predicate_rejects_garbage() {
    // 至少不应把任意字符串识别为已安装
    assert!(!font_family_installed(
        "__lyric_for_musicfox_definitely_missing_font__"
    ));
}
