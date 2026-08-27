//! 设置进程表单 / 防抖 / .tmp 崩溃恢复矩阵测试
//!
//! Oracle 计划：
//! - 缺失 config bootstrap
//! - 有效 tmp 优先
//! - 无效 tmp 恢复
//! - 500ms 防抖边界
//! - 防抖不清 dirty
//! - 保存写正式 config + 重写 tmp
//! - 非法保存保留正式 config
//! - discard 删除 tmp
//!
//! ⚠️ `flush_and_save_now` 会调 `notify_reload_preserving_wt`。
//!    集成测试运行在 target/debug/deps/ 下，pipe/mod.rs 会自动跳过重启，避免派生孤儿测试进程。

use std::time::{Duration, Instant};

use lyric_for_musicfox::config::{Config, WindowConfig};
use lyric_for_musicfox::settings::form::{working_bak_path, working_tmp_path, FormState};
use lyric_for_musicfox::settings::validate::validate_all;
use lyric_for_musicfox::{load_config, save_config};

fn fresh_paths(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let config_path = dir.join("config.toml");
    let tmp_path = working_tmp_path(&config_path);
    (config_path, tmp_path)
}

fn write_config(path: &std::path::Path, c: &Config) {
    save_config(c, path).unwrap();
}

#[test]
fn bootstrap_creates_default_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg_path, tmp_path) = fresh_paths(dir.path());
    let mut form = FormState::new(cfg_path.clone()).unwrap();
    form.bootstrap().unwrap();
    assert!(cfg_path.exists());
    assert!(tmp_path.exists());
    assert_eq!(load_config(&cfg_path).unwrap(), Config::default());
}

#[test]
fn bootstrap_prefers_valid_tmp_over_config() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg_path, tmp_path) = fresh_paths(dir.path());
    let mut draft_cfg = Config::default();
    draft_cfg.window = WindowConfig {
        width: 900,
        ..WindowConfig::default()
    };
    write_config(&cfg_path, &draft_cfg);
    // 写 tmp 与 cfg 不一致
    let mut draft_tmp = Config::default();
    draft_tmp.window.width = 1234;
    save_config(&draft_tmp, &tmp_path).unwrap();

    let mut form = FormState::new(cfg_path.clone()).unwrap();
    form.bootstrap().unwrap();
    assert_eq!(form.draft.window.width, 1234, "应以 tmp 为准");
    assert!(form.dirty);
}

#[test]
fn bootstrap_discards_invalid_tmp_and_recovers_from_config() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg_path, tmp_path) = fresh_paths(dir.path());
    write_config(&cfg_path, &Config::default());
    std::fs::write(&tmp_path, "this is not valid toml {{{").unwrap();

    let mut form = FormState::new(cfg_path.clone()).unwrap();
    form.bootstrap().unwrap();
    assert_eq!(form.draft, Config::default());
    assert!(tmp_path.exists(), "重新写入的 tmp 应存在");
}

#[test]
fn debounce_500ms_writes_tmp_but_does_not_clear_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg_path, tmp_path) = fresh_paths(dir.path());
    let mut form = FormState::new(cfg_path.clone()).unwrap();
    form.bootstrap().unwrap();

    form.draft.window.width = 950;
    form.mark_dirty();
    assert!(form.dirty);

    // 200ms 后：未到期，不应写 tmp
    std::thread::sleep(Duration::from_millis(200));
    form.flush_tmp_if_due().unwrap();
    assert!(
        !tmp_path.exists()
            || std::fs::read_to_string(&tmp_path)
                .unwrap()
                .contains("width = 800")
    );

    // 等到 500ms 之后
    std::thread::sleep(Duration::from_millis(400));
    form.flush_tmp_if_due().unwrap();
    assert!(tmp_path.exists());
    let loaded = load_config(&tmp_path).unwrap();
    assert_eq!(loaded.window.width, 950, "tmp 应包含新值");
    assert!(form.dirty, "防抖写 tmp 不应清 dirty");
}

#[test]
fn save_writes_config_atomically_and_recreates_tmp() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg_path, tmp_path) = fresh_paths(dir.path());
    let mut form = FormState::new(cfg_path.clone()).unwrap();
    form.bootstrap().unwrap();

    form.draft.window.width = 1024;
    form.mark_dirty();

    form.flush_and_save_now().unwrap();
    assert_eq!(load_config(&cfg_path).unwrap().window.width, 1024);
    assert!(tmp_path.exists(), "提交后应重建 tmp");
    assert_eq!(
        load_config(&tmp_path).unwrap().window.width,
        1024,
        "tmp 应等于刚提交的配置"
    );
    assert!(!form.dirty);
}

#[test]
fn invalid_save_keeps_formal_config_intact() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg_path, _) = fresh_paths(dir.path());
    write_config(&cfg_path, &Config::default());

    let mut form = FormState::new(cfg_path.clone()).unwrap();
    form.bootstrap().unwrap();

    // 把 font_color 设非法
    form.draft.lyric_style.font_color = "#zzz".into();
    form.mark_dirty();
    let err = form.flush_and_save_now().unwrap_err();
    assert!(err.iter().any(|(k, _)| k == "font_color"));

    // 正式 config 仍是默认
    assert_eq!(load_config(&cfg_path).unwrap(), Config::default());
}

#[test]
fn discard_tmp_removes_file_and_clears_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg_path, tmp_path) = fresh_paths(dir.path());
    let mut form = FormState::new(cfg_path.clone()).unwrap();
    form.bootstrap().unwrap();
    form.draft.window.width = 950;
    form.mark_dirty();
    // 触发防抖写 tmp
    std::thread::sleep(Duration::from_millis(600));
    form.flush_tmp_if_due().unwrap();
    assert!(tmp_path.exists());

    form.discard_tmp().unwrap();
    assert!(!tmp_path.exists());
    assert!(!form.dirty);
}

#[test]
fn validate_all_returns_multiple_errors_without_short_circuit() {
    let mut bad = Config::default();
    bad.window.width = 1; // too small
    bad.lyric_style.font_color = "not a color".into();
    bad.lyric_style.font_size = 999.0; // too big
    let errors = validate_all(&bad);
    let keys: Vec<&str> = errors.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.contains(&"width"));
    assert!(keys.contains(&"font_color"));
    assert!(keys.contains(&"font_size"));
}

#[test]
fn working_paths_use_expected_suffixes() {
    let p = std::path::Path::new("/a/b/config.toml");
    assert_eq!(
        working_tmp_path(p),
        std::path::Path::new("/a/b/config.toml.tmp")
    );
    assert_eq!(
        working_bak_path(p),
        std::path::Path::new("/a/b/config.toml.bak")
    );
}

#[test]
fn debounce_does_not_reset_dirty_too_eagerly() {
    // 在 elapsed < DEBOUNCE 时 mark_dirty 不该把 dirty 置 false
    let dir = tempfile::tempdir().unwrap();
    let (cfg_path, _) = fresh_paths(dir.path());
    let mut form = FormState::new(cfg_path).unwrap();
    form.bootstrap().unwrap();
    form.mark_dirty();
    let t0 = Instant::now();
    form.last_change = Some(t0);
    form.flush_tmp_if_due().unwrap();
    assert!(form.dirty);
    assert!(form.last_change.is_some());
}

#[test]
fn broken_config_triggers_parse_modal_and_resets_with_backup() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg_path, tmp_path) = fresh_paths(dir.path());
    let bak_path = working_bak_path(&cfg_path);

    // 写入损坏的 toml
    std::fs::write(&cfg_path, "invalid toml syntax [[[[").unwrap();

    let mut form = FormState::new(cfg_path.clone()).unwrap();
    let outcome = form.bootstrap().unwrap();
    assert_eq!(
        outcome,
        lyric_for_musicfox::settings::form::BootstrapOutcome::ConfigInvalid
    );
    assert!(form.parse_modal.is_some(), "应触发 parse_modal");

    // 点击重置
    form.reset_to_default_with_backup().unwrap();
    assert!(bak_path.exists(), "备份文件 .bak 应存在");
    assert_eq!(
        std::fs::read_to_string(&bak_path).unwrap(),
        "invalid toml syntax [[[["
    );
    assert!(cfg_path.exists());
    assert!(tmp_path.exists());
    assert_eq!(load_config(&cfg_path).unwrap(), Config::default());
    assert!(!form.dirty);
}

#[test]
fn parse_pos_response_handles_various_formats() {
    use lyric_for_musicfox::settings::parse_pos_response;

    assert_eq!(
        parse_pos_response("100,200\n").unwrap(),
        (Some(100), Some(200))
    );
    assert_eq!(
        parse_pos_response("POS 300,400\r\n").unwrap(),
        (Some(300), Some(400))
    );
    assert_eq!(parse_pos_response("EMPTY,EMPTY\n").unwrap(), (None, None));
    assert_eq!(parse_pos_response("EMPTY\n").unwrap(), (None, None));
    assert_eq!(parse_pos_response("100,EMPTY\n").unwrap(), (Some(100), None));
}

#[test]
fn toast_lifecycle_visible_and_expires() {
    let dir = tempfile::tempdir().unwrap();
    let (cfg_path, _) = fresh_paths(dir.path());
    let mut form = FormState::new(cfg_path).unwrap();

    assert!(form.toast_visible().is_none());
    form.show_toast("已发送重载通知");
    assert_eq!(form.toast_visible(), Some("已发送重载通知"));
}
