//! 配置原子写：.tmp → rename 为 config.toml
//! **库 API**：主进程不得调用（写盘是设置进程的职责）
//!
//! P0 `save` 签名保持不变。P1 新增工作副本 API：
//! `save_tmp` / `commit_tmp` / `copy_to_tmp`。

use super::Config;
use crate::error::AppError;
use std::path::Path;

/// 原子写配置到 path（path 期望是 config.toml）
/// 返回写入的字节数
pub fn save(config: &Config, path: &Path) -> Result<usize, AppError> {
    let tmp_path = path.with_extension("toml.tmp");
    let content =
        toml::to_string_pretty(config).map_err(|e| AppError::ConfigSerialize(e.to_string()))?;
    std::fs::write(&tmp_path, &content)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(content.len())
}

/// 将草稿序列化写入工作副本 `.tmp`（不碰正式 config.toml）
pub fn save_tmp(config: &Config, tmp_path: &Path) -> Result<usize, AppError> {
    if let Some(parent) = tmp_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let content =
        toml::to_string_pretty(config).map_err(|e| AppError::ConfigSerialize(e.to_string()))?;
    std::fs::write(tmp_path, &content)?;
    Ok(content.len())
}

/// 将工作副本原子替换为正式 `config.toml`，随后重新拷贝正式文件为新的 `.tmp`。
///
/// 若进程恰在二次拷贝前崩溃，下次启动没有 `.tmp`，按规范从有效正式配置重建。
pub fn commit_tmp(tmp_path: &Path, config_path: &Path) -> Result<(), AppError> {
    replace_file(tmp_path, config_path)?;
    copy_to_tmp(config_path, tmp_path)?;
    Ok(())
}

/// 把正式配置复制为工作副本
pub fn copy_to_tmp(config_path: &Path, tmp_path: &Path) -> Result<u64, AppError> {
    if let Some(parent) = tmp_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(std::fs::copy(config_path, tmp_path)?)
}

fn replace_file(from: &Path, to: &Path) -> Result<(), AppError> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_e) if to.exists() => {
            // Windows 上 dest 已存在时 std::fs::rename 可能失败。
            // 同目录 remove + rename 作为可恢复回退；Linux rename 本就会替换。
            std::fs::remove_file(to)?;
            std::fs::rename(from, to)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
