//! 配置加载：文件不存在 → Config::default()；解析失败 → AppError::ConfigParse

use super::Config;
use crate::error::AppError;
use std::path::Path;

/// 加载 config.toml：
/// - 文件不存在 → Ok(Config::default())
/// - 解析失败 → AppError::ConfigParse（exit code 1）
pub fn load(path: &Path) -> Result<Config, AppError> {
    if !path.exists() {
        return Ok(Config::default());
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::ConfigParse(format!("read error: {e}")))?;

    toml::from_str(&content).map_err(|e| AppError::ConfigParse(format!("parse error: {e}")))
}
