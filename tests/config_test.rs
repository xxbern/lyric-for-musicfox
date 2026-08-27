//! config 模块单元测试
//! 覆盖：默认值 round-trip、Option 字段空串 ↔ None、缺失字段、非法值

use lyric_for_musicfox::config::{Config, LyricStyleConfig, WindowConfig};
use lyric_for_musicfox::{load_config, save_config};
use std::fs;

#[test]
fn test_config_default_roundtrip() {
    let original = Config::default();
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");

    save_config(&original, &path).unwrap();
    let loaded = load_config(&path).unwrap();

    // PartialEq derive → 整体比较
    assert_eq!(original, loaded);
}

#[test]
fn test_pos_none_serializes_to_empty_string() {
    let config = Config::default(); // pos_x = None, pos_y = None
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");

    save_config(&config, &path).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.contains(r#"pos_x = """#),
        "pos_x=None should serialize to empty string; got:\n{content}"
    );
    assert!(
        content.contains(r#"pos_y = """#),
        "pos_y=None should serialize to empty string; got:\n{content}"
    );
}

#[test]
fn test_pos_some_roundtrip() {
    let mut config = Config::default();
    config.window.pos_x = Some(100);
    config.window.pos_y = Some(-200);

    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    save_config(&config, &path).unwrap();
    let loaded = load_config(&path).unwrap();

    assert_eq!(Some(100), loaded.window.pos_x);
    assert_eq!(Some(-200), loaded.window.pos_y);
}

#[test]
fn test_pos_missing_field_becomes_none() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
[window]
width = 800
height = 80
stay_on_top = true
"#,
    )
    .unwrap();

    let loaded = load_config(&path).unwrap();
    assert_eq!(None, loaded.window.pos_x);
    assert_eq!(None, loaded.window.pos_y);
}

#[test]
fn test_pos_invalid_string_rejected() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
[window]
width = 800
height = 80
pos_x = "not_an_integer"
"#,
    )
    .unwrap();

    let result = load_config(&path);
    assert!(result.is_err());
    assert_eq!(1, result.unwrap_err().exit_code());
}

#[test]
fn test_outline_color_missing_becomes_some() {
    // 缺失 → default_outline_color() → Some("#000000")
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
[lyric_style]
font_family = "Microsoft YaHei"
font_size = 24.0
"#,
    )
    .unwrap();

    let loaded = load_config(&path).unwrap();
    assert_eq!(
        Some("#000000".to_string()),
        loaded.lyric_style.font_outline_color
    );
}

#[test]
fn test_outline_color_empty_string_becomes_none() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
[lyric_style]
font_outline_color = ""
"#,
    )
    .unwrap();

    let loaded = load_config(&path).unwrap();
    assert_eq!(None, loaded.lyric_style.font_outline_color);
}

#[test]
fn test_outline_color_some_roundtrip() {
    let mut config = Config::default();
    config.lyric_style.font_outline_color = Some("#ff0000".to_string());

    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    save_config(&config, &path).unwrap();
    let loaded = load_config(&path).unwrap();

    assert_eq!(
        Some("#ff0000".to_string()),
        loaded.lyric_style.font_outline_color
    );
}

#[test]
fn test_all_fields_missing_uses_defaults() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    fs::write(&path, "").unwrap();

    let loaded = load_config(&path).unwrap();
    assert_eq!(800, loaded.window.width);
    assert_eq!(80, loaded.window.height);
    assert_eq!(16501, loaded.system.receive_port);
    assert_eq!("Microsoft YaHei", loaded.lyric_style.font_family);
    assert_eq!(24.0, loaded.lyric_style.font_size);
    assert_eq!(true, loaded.window.stay_on_top);
    assert_eq!(false, loaded.window.locked);
}

#[test]
fn test_parse_error_returns_err() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    fs::write(&path, "this is not valid toml {{{").unwrap();

    let result = load_config(&path);
    assert!(result.is_err());
    assert_eq!(1, result.unwrap_err().exit_code());
}

#[test]
fn test_window_config_default() {
    let w = WindowConfig::default();
    assert_eq!(800, w.width);
    assert_eq!(80, w.height);
    assert_eq!(None, w.pos_x);
    assert_eq!(None, w.pos_y);
    assert_eq!(true, w.stay_on_top);
    assert_eq!(true, w.frame_less);
    assert_eq!(false, w.locked);
}

#[test]
fn test_lyric_style_config_default() {
    let s = LyricStyleConfig::default();
    assert_eq!("Microsoft YaHei", s.font_family);
    assert_eq!(24.0, s.font_size);
    assert_eq!(false, s.font_bold);
    assert_eq!(false, s.font_italic);
    assert_eq!("#ffffff", s.font_color);
    assert_eq!(Some("#000000".to_string()), s.font_outline_color);
    assert_eq!(1, s.font_outline_width);
}

#[test]
fn test_system_config_log_enabled() {
    let config = Config::default();
    assert_eq!(false, config.system.log_enabled);

    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    save_config(&config, &path).unwrap();
    let loaded = load_config(&path).unwrap();
    assert_eq!(false, loaded.system.log_enabled);
}

#[test]
fn test_frame_less_skipped_serializing() {
    let config = Config::default();
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    save_config(&config, &path).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(
        !content.contains("frame_less"),
        "frame_less should be skipped when serializing, got:\n{}",
        content
    );
}

#[test]
fn test_wt_config_defaults_and_roundtrip() {
    let config = Config::default();
    assert_eq!(
        config.wt.musicfox_path,
        "C:\\Users\\xx\\app\\musicfox\\musicfox.exe"
    );
    assert_eq!(config.wt.app_dir, "C:\\Users\\xx\\app");
    assert_eq!(config.wt.title, "MusicFoxTerminal");

    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    save_config(&config, &path).unwrap();

    let loaded = load_config(&path).unwrap();
    assert_eq!(loaded.wt, config.wt);
}

#[test]
fn test_wt_config_missing_section_falls_back_to_default() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    // 写入不包含 [wt] 的旧版 config.toml
    fs::write(
        &path,
        r#"
[window]
width = 800
height = 80

[lyric_style]
font_family = "Microsoft YaHei"
font_size = 24.0

[system]
receive_port = 16501
send_port = 16502
"#,
    )
    .unwrap();

    let loaded = load_config(&path).unwrap();
    assert_eq!(loaded.wt, lyric_for_musicfox::config::WtConfig::default());
}
