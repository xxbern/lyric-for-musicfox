//! 表单状态：bootstrap / 防抖写 .tmp / 保存原子提交

use crate::config::save::{commit_tmp, copy_to_tmp, save_tmp};
use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::load_config;
use crate::save_config;
use crate::settings::validate::{normalize_colors_in_place, validate_all};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(500);
pub const TOAST_DURATION: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapOutcome {
    Loaded,
    CreatedDefault,
    RecoveredFromTmp,
    RecoveredFromConfigAfterBadTmp,
    ConfigInvalid,
}

#[derive(Debug, Clone)]
pub struct ParseModal {
    pub error: String,
}

pub struct FormState {
    pub draft: Config,
    pub dirty: bool,
    pub last_change: Option<Instant>,
    pub tmp_path: PathBuf,
    pub config_path: PathBuf,
    pub parse_modal: Option<ParseModal>,
    pub toast_until: Option<Instant>,
    pub toast_text: Option<String>,
}

impl FormState {
    pub fn new(config_path: PathBuf) -> AppResult<Self> {
        let tmp_path = working_tmp_path(&config_path);
        Ok(Self {
            draft: Config::default(),
            dirty: false,
            last_change: None,
            tmp_path,
            config_path,
            parse_modal: None,
            toast_until: None,
            toast_text: None,
        })
    }

    pub fn bak_path(&self) -> PathBuf {
        working_bak_path(&self.config_path)
    }

    pub fn bootstrap(&mut self) -> AppResult<BootstrapOutcome> {
        let tmp_exists = self.tmp_path.exists();
        let config_exists = self.config_path.exists();

        if tmp_exists {
            match load_existing(&self.tmp_path) {
                Ok(draft) => {
                    self.draft = draft;
                    if config_exists {
                        match load_existing(&self.config_path) {
                            Ok(formal) => self.dirty = formal != self.draft,
                            Err(_) => self.dirty = true,
                        }
                    } else {
                        self.dirty = true;
                    }
                    return Ok(BootstrapOutcome::RecoveredFromTmp);
                }
                Err(e) => {
                    log::warn!(
                        "discarding invalid working copy {}: {e}",
                        self.tmp_path.display()
                    );
                    let _ = std::fs::remove_file(&self.tmp_path);
                }
            }
        }

        if config_exists {
            match load_existing(&self.config_path) {
                Ok(config) => {
                    self.draft = config;
                    copy_to_tmp(&self.config_path, &self.tmp_path)?;
                    self.dirty = false;
                    if tmp_exists {
                        return Ok(BootstrapOutcome::RecoveredFromConfigAfterBadTmp);
                    }
                    return Ok(BootstrapOutcome::Loaded);
                }
                Err(e) => {
                    self.parse_modal = Some(ParseModal {
                        error: e.to_string(),
                    });
                    return Ok(BootstrapOutcome::ConfigInvalid);
                }
            }
        }

        self.create_default_files()?;
        self.show_toast("正在创建配置文件");
        Ok(BootstrapOutcome::CreatedDefault)
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.last_change = Some(Instant::now());
    }

    /// 仅由 SettingsApp::update() 调用。到期只写 .tmp，故意不清除 dirty。
    pub fn flush_tmp_if_due(&mut self) -> AppResult<()> {
        let Some(t) = self.last_change else {
            return Ok(());
        };
        if !self.dirty {
            return Ok(());
        }
        if t.elapsed() < DEBOUNCE {
            return Ok(());
        }
        save_tmp(&self.draft, &self.tmp_path)?;
        self.last_change = None;
        Ok(())
    }

    /// 立即写 .tmp → 校验 → 原子提交 → 重建 .tmp。仅全成功后清 dirty。
    pub fn flush_and_save_now(&mut self) -> Result<(), Vec<(String, String)>> {
        self.last_change = None;
        if let Err(e) = save_tmp(&self.draft, &self.tmp_path) {
            return Err(vec![("save".into(), e.to_string())]);
        }
        let errors = validate_all(&self.draft);
        if !errors.is_empty() {
            return Err(errors);
        }
        normalize_colors_in_place(&mut self.draft);
        if let Err(e) = save_tmp(&self.draft, &self.tmp_path) {
            return Err(vec![("save".into(), e.to_string())]);
        }
        if let Err(e) = commit_tmp(&self.tmp_path, &self.config_path) {
            return Err(vec![("save".into(), e.to_string())]);
        }
        self.dirty = false;
        self.last_change = None;

        // 保存后重启 lyric 主进程（保留 WT 不中断）
        std::thread::spawn(move || {
            crate::pipe::reload::notify_reload_preserving_wt();
        });

        Ok(())
    }

    pub fn discard_tmp(&mut self) -> AppResult<()> {
        if self.tmp_path.exists() {
            std::fs::remove_file(&self.tmp_path)?;
        }
        self.dirty = false;
        self.last_change = None;
        Ok(())
    }

    pub fn reset_to_default_with_backup(&mut self) -> AppResult<()> {
        if self.config_path.exists() {
            let bak = self.bak_path();
            std::fs::copy(&self.config_path, &bak)?;
        }
        self.draft = Config::default();
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        save_config(&self.draft, &self.config_path)?;
        copy_to_tmp(&self.config_path, &self.tmp_path)?;
        self.dirty = false;
        self.last_change = None;
        self.parse_modal = None;
        Ok(())
    }

    fn create_default_files(&mut self) -> AppResult<()> {
        self.draft = Config::default();
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        save_config(&self.draft, &self.config_path)?;
        copy_to_tmp(&self.config_path, &self.tmp_path)?;
        self.dirty = false;
        Ok(())
    }

    pub fn show_toast(&mut self, text: &str) {
        self.toast_text = Some(text.to_string());
        self.toast_until = Some(Instant::now() + TOAST_DURATION);
    }

    pub fn toast_visible(&self) -> Option<&str> {
        let until = self.toast_until?;
        if Instant::now() < until {
            self.toast_text.as_deref()
        } else {
            None
        }
    }
}

pub fn working_tmp_path(config_path: &Path) -> PathBuf {
    config_path.with_extension("toml.tmp")
}

pub fn working_bak_path(config_path: &Path) -> PathBuf {
    config_path.with_extension("toml.bak")
}

fn load_existing(path: &Path) -> Result<Config, AppError> {
    if !path.exists() {
        return Err(AppError::ConfigParse(format!("missing {}", path.display())));
    }
    load_config(path)
}
