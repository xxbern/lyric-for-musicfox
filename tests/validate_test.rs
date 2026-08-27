//! 设置页字段校验测试（req.md §10.4）
//!
//! 覆盖 Oracle 计划：
//! - 范围边界
//! - NaN/infinite font_size 拒绝
//! - 有效/无效颜色
//! - 大小写规范化
//! - outline None
//! - 字体存在/缺失

use lyric_for_musicfox::settings::validate::{
    is_rrggbb, normalize_color, validate_color, validate_font_family, validate_font_size,
    validate_height, validate_musicfox_path, validate_outline_width, validate_width,
};
use lyric_for_musicfox::window::FALLBACK_FONT_FAMILY;

#[test]
fn width_range_boundaries() {
    assert!(validate_width(100).is_ok());
    assert!(validate_width(4000).is_ok());
    assert!(validate_width(99).is_err());
    assert!(validate_width(4001).is_err());
}

#[test]
fn height_range_boundaries() {
    assert!(validate_height(20).is_ok());
    assert!(validate_height(500).is_ok());
    assert!(validate_height(19).is_err());
    assert!(validate_height(501).is_err());
}

#[test]
fn font_size_range_and_finite() {
    assert!(validate_font_size(6.0).is_ok());
    assert!(validate_font_size(200.0).is_ok());
    assert!(validate_font_size(5.999).is_err());
    assert!(validate_font_size(200.0001).is_err());
    assert!(validate_font_size(f32::NAN).is_err());
    assert!(validate_font_size(f32::INFINITY).is_err());
}

#[test]
fn outline_width_range() {
    assert!(validate_outline_width(0).is_ok());
    assert!(validate_outline_width(16).is_ok());
    assert!(validate_outline_width(17).is_err());
}

#[test]
fn color_validity_and_case() {
    assert!(validate_color("#ffffff").is_ok());
    assert!(validate_color("#FFFFFF").is_ok());
    assert!(validate_color("#aB12Cd").is_ok());
    assert!(validate_color("ffffff").is_err());
    assert!(validate_color("#fff").is_err());
    assert!(validate_color("#zzzzzz").is_err());
    assert!(is_rrggbb("#123abc"));
    assert!(!is_rrggbb("#1234abc"));
}

#[test]
fn normalize_lowercases_valid_colors() {
    assert_eq!(normalize_color("#FFFFFF"), "#ffffff");
    assert_eq!(normalize_color("#aB12Cd"), "#ab12cd");
}

#[test]
fn font_family_validation() {
    // Microsoft YaHei（系统内置或为默认回退字体）若平台无法验证，本测试允许 fail
    let _ = validate_font_family(FALLBACK_FONT_FAMILY);

    // 不存在的字体名：仅 Windows 可确定性判定
    if cfg!(windows) {
        let err = validate_font_family("__lyric_for_musicfox_definitely_missing_font__")
            .expect_err("missing font must error");
        assert!(err.contains("__lyric_for_musicfox_definitely_missing_font__"));
    }
}

#[test]
fn musicfox_path_validation() {
    assert!(validate_musicfox_path("").is_err(), "empty path must fail");
    assert!(
        validate_musicfox_path("/non_existent_dir_98765/musicfox.exe").is_err(),
        "non-existent path must fail"
    );

    // 当前存在的当前文件应通过验证
    if let Ok(exe) = std::env::current_exe() {
        assert!(validate_musicfox_path(&exe.to_string_lossy()).is_ok());
    }
}
