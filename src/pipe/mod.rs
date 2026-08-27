// src/pipe/mod.rs
pub use crate::protocol::{get_pipe_path as get_pipe_name, get_session_id};

pub mod reload {
    use std::path::PathBuf;
    use std::sync::Arc;
    use crate::context::AppContext;

    pub fn start_server(config_path: PathBuf, ctx: Arc<AppContext>) {
        let _ = crate::platform::current().start_reload(ctx, config_path);
    }

    /// 保存配置后重启歌词主进程，同时保留 WT + go-musicfox 不中断。
    ///
    /// 行为：
    ///   1. 先撤销当前进程的 WT 事件钩子（不发送 WM_CLOSE 关闭 WT 窗口，
    ///      让 WindowsTerminal.exe + go-musicfox 继续运行）。
    ///   2. 拉起全新的 lyric 主进程；新进程启动 1s 后会自动调
    ///      `WtService::init_delayed_check`，重新找到同一个 WT 窗口并接管样式与钩子。
    ///
    /// 设置侧 Toast 只需表达"配置已保存，lyric 主进程将重启"，无需在线/离线区分。
    ///
    /// 测试环境下设 `LYRIC_NO_RESTART=1` 可跳过重启（避免派生孤儿测试进程抢资源）。
    pub fn notify_reload_preserving_wt() {
        // 0. 环境变量门控：测试跳过
        if std::env::var_os("LYRIC_NO_RESTART").is_some() {
            log::debug!("notify_reload_preserving_wt skipped (LYRIC_NO_RESTART set)");
            return;
        }

        // 0.1 cargo test 集成测试 binary 运行路径位于 target/debug/deps/，
        // 路径含 "deps/" 时也跳过（避免未被测试代码读到环境变量仍误重启）。
        if let Ok(exe) = std::env::current_exe() {
            if exe.to_string_lossy().contains("/deps/") || exe.to_string_lossy().contains("\\deps\\") {
                log::debug!("notify_reload_preserving_wt skipped (cargo test binary detected)");
                return;
            }
        }

        // 1. 卸载当前进程的 WT 事件钩子（如果已安装）
        let title = crate::config::default_wt_title();
        crate::platform::current().uninstall_wt_minimize_hook(&title);

        // 2. 重启 lyric 主进程
        #[cfg(windows)]
        crate::platform::windows::restart_lyric_app();
        #[cfg(not(windows))]
        crate::platform::stub::restart_lyric_app();
    }

    /// 向后兼容别名：旧调用方使用 `notify_reload()`，现等同于 `notify_reload_preserving_wt`。
    #[inline]
    pub fn notify_reload() {
        notify_reload_preserving_wt();
    }
}

pub mod pos {
    use std::sync::Arc;
    use crate::context::AppContext;
    use crate::protocol::{platform_pipe_path, get_session_id, IpcMessage};

    pub fn start_server(ctx: Arc<AppContext>) {
        let _ = crate::platform::current().start_pos(ctx);
    }

    pub fn query_pos() -> Option<(i32, i32)> {
        let sid = get_session_id();
        let name = platform_pipe_path("pos", sid);
        let req = IpcMessage::GetPosition.to_bytes();
        if let Ok(resp) = crate::platform::current().send(&name, &req, 1000) {
            if let Ok(IpcMessage::PositionResponse(coords)) = IpcMessage::parse(&resp) {
                return coords;
            }
        }
        None
    }
}

pub mod presence {
    pub fn start_server() {
        let _ = crate::platform::current().start_presence();
    }

    pub fn try_activate_existing() -> Result<(), String> {
        let pid = std::process::id();
        crate::platform::current().try_activate(pid)
    }
}
