//! 统一错误类型 + Exit code 映射（dev.md §9）

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("config parse error: {0}")]
    ConfigParse(String),

    #[error("config serialize error: {0}")]
    ConfigSerialize(String),

    #[error("another instance is running")]
    AnotherInstance,

    #[error("mutex create failed: {0}")]
    MutexCreate(String),

    #[error("port {0} already in use")]
    PortInUse(u16),

    #[error("settings mode is available from P1")]
    SettingsUnavailable,

    #[error("pos-client query timed out")]
    PosPipeTimeout,

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl AppError {
    /// 映射到 dev.md §9 Exit code 字典
    pub fn exit_code(&self) -> i32 {
        match self {
            AppError::ConfigParse(_) => 1,
            AppError::ConfigSerialize(_) => 1,
            AppError::AnotherInstance => 6,
            AppError::MutexCreate(_) => 3,
            AppError::PortInUse(_) => 1,
            AppError::SettingsUnavailable => 4,
            // 设置 UI 边界吞掉并降级为磁盘坐标；不是进程退出原因
            AppError::PosPipeTimeout => 0,
            AppError::Json(_) => 1,
            AppError::Io(_) => 2, // dev.md §9: 配置路径不可写 exit 2
        }
    }

    /// 是否向 stderr 打印错误消息
    pub fn print_to_stderr(&self) {
        match self {
            AppError::ConfigParse(msg) => eprintln!("config parse error: {msg}"),
            AppError::ConfigSerialize(msg) => eprintln!("config serialize error: {msg}"),
            AppError::AnotherInstance => eprintln!("another instance running"),
            AppError::MutexCreate(msg) => eprintln!("mutex create failed: {msg}"),
            AppError::PortInUse(port) => eprintln!("port {port} already in use"),
            AppError::SettingsUnavailable => eprintln!("settings mode is available from P1"),
            AppError::PosPipeTimeout => {}
            AppError::Json(e) => eprintln!("json error: {e}"),
            AppError::Io(e) => eprintln!("io error: {e}"),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
