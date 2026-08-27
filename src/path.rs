//! 路径解析：%APPDATA%/lyric-for-musicfox/ （Windows）；非 Windows 平台用 dirs::data_dir()
//! 失败时返回 io::Error，main 中映射为 exit code 2（dev.md §9）

use std::path::PathBuf;

/// 返回应用数据目录（不自动创建）
pub fn app_data_dir() -> std::io::Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cannot resolve user data directory",
        )
    })?;
    Ok(base.join("lyric-for-musicfox"))
}

/// 配置文件路径
pub fn config_path() -> std::io::Result<PathBuf> {
    Ok(app_data_dir()?.join("config.toml"))
}

/// 日志文件路径
pub fn log_path() -> std::io::Result<PathBuf> {
    Ok(app_data_dir()?.join("log.txt"))
}

/// 复刻 go-musicfox `DataDir()`：cookie / db / logo.png 等用户数据所在目录
pub fn resolve_musicfox_data_dir() -> Option<PathBuf> {
    // 1. portable 模式：MUSICFOX_ROOT 非空 → <MUSICFOX_ROOT>/data
    if let Ok(root) = std::env::var("MUSICFOX_ROOT") {
        let trimmed = root.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join("data"));
        }
    }
    // 2. XDG 模式：XDG_DATA_HOME/go-musicfox
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        let trimmed = xdg_data.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join("go-musicfox"));
        }
    }
    // 3. Windows 默认：LOCALAPPDATA\go-musicfox (注意：不是 Roaming APPDATA)
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let trimmed = local.trim();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed).join("go-musicfox"));
            }
        }
        // 4. Windows 兜底：USERPROFILE\AppData\Local\go-musicfox
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let trimmed = profile.trim();
            if !trimmed.is_empty() {
                return Some(
                    PathBuf::from(trimmed)
                        .join("AppData")
                        .join("Local")
                        .join("go-musicfox"),
                );
            }
        }
    }
    // 非 Windows 默认兜底：~/.local/share/go-musicfox
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = dirs::home_dir() {
            return Some(home.join(".local").join("share").join("go-musicfox"));
        }
    }
    None
}

/// 返回 UI 展示用的 go-musicfox 数据目录提示文本
pub fn musicfox_data_dir_display() -> String {
    if let Some(path) = resolve_musicfox_data_dir() {
        path.display().to_string()
    } else {
        "无法定位".to_string()
    }
}
