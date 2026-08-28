//! Windows Terminal (WT) 托管服务：启动、样式改造、透明显隐切换与最小化拦截。

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use crate::config::WtConfig;
use crate::context::AppContext;

static LAST_TOGGLE: OnceLock<Mutex<Instant>> = OnceLock::new();
static IS_LAUNCHING: OnceLock<Mutex<bool>> = OnceLock::new();

fn get_last_toggle() -> &'static Mutex<Instant> {
    LAST_TOGGLE.get_or_init(|| Mutex::new(Instant::now() - Duration::from_secs(10)))
}

fn get_is_launching() -> &'static Mutex<bool> {
    IS_LAUNCHING.get_or_init(|| Mutex::new(false))
}

#[derive(Clone)]
pub struct WtService {
    ctx: Arc<AppContext>,
}

impl WtService {
    pub fn new(ctx: Arc<AppContext>) -> Self {
        Self { ctx }
    }

    /// 托盘单击或命令触发 WT 显隐切换 / 启动
    pub fn toggle(&self, config: &WtConfig) {
        // 1. 全局 400ms 防抖，防止系统连击事件误触发多次
        {
            let mut last = get_last_toggle().lock().unwrap();
            if last.elapsed() < Duration::from_millis(400) {
                log::debug!("WtService::toggle debounced");
                return;
            }
            *last = Instant::now();
        }

        // 2. 检查是否正在启动过程中
        {
            let launching = get_is_launching().lock().unwrap();
            if *launching {
                log::info!("WT is currently launching, ignore duplicate toggle");
                return;
            }
        }

        // 3. 动态读取磁盘最新配置，确保拿到用户在设置界面最新保存的路径
        let current_cfg = crate::services::config::ConfigService::load_or_default()
            .unwrap_or_else(|_| self.ctx.get_config());
        let effective_config = if !current_cfg.wt.musicfox_path.is_empty() {
            &current_cfg.wt
        } else {
            config
        };

        if let Some(hwnd) = crate::platform::current().find_wt_window(&effective_config.title) {
            if crate::platform::current().is_wt_hidden(hwnd) {
                crate::platform::current().set_wt_alpha(hwnd, 255);
                crate::platform::current().activate_wt(hwnd);
            } else {
                crate::platform::current().set_wt_alpha(hwnd, 0);
            }
        } else {
            // 校验 musicfox 可执行文件有效性
            if let Err(err) = crate::settings::validate::validate_musicfox_path(&effective_config.musicfox_path) {
                log::warn!("Cannot launch WT: musicfox executable invalid: {err}");
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).arg("--settings").spawn();
                }
                let desc = format!("无法启动 go-musicfox：{err}。\n\n已为您自动打开设置界面，请在设置中指定正确的 musicfox.exe 路径。");
                std::thread::spawn(move || {
                    rfd::MessageDialog::new()
                        .set_title("go-musicfox 未找到")
                        .set_description(&desc)
                        .set_level(rfd::MessageLevel::Warning)
                        .set_buttons(rfd::MessageButtons::Ok)
                        .show();
                });
                return;
            }

            // 加锁标记正在启动
            {
                let mut launching = get_is_launching().lock().unwrap();
                *launching = true;
            }

            // 智能推导工作目录：优先使用 musicfox.exe 所在的父目录
            let musicfox_path_buf = std::path::PathBuf::from(&effective_config.musicfox_path);
            let app_dir = if let Some(parent) = musicfox_path_buf.parent() {
                if parent.exists() {
                    parent.to_string_lossy().to_string()
                } else {
                    effective_config.app_dir.clone()
                }
            } else {
                effective_config.app_dir.clone()
            };

            log::info!(
                "Launching WT: path={}, dir={}, title={}",
                effective_config.musicfox_path,
                app_dir,
                effective_config.title
            );

            if let Err(e) = crate::platform::current().launch_wt(
                &app_dir,
                &effective_config.title,
                &effective_config.musicfox_path,
            ) {
                log::warn!("Failed to launch WT: {e}");
                let mut launching = get_is_launching().lock().unwrap();
                *launching = false;
                return;
            }

            let title = effective_config.title.clone();
            std::thread::spawn(move || {
                // 等待 WT 窗口创建并渲染
                std::thread::sleep(Duration::from_secs(1));
                if let Some(hwnd) = crate::platform::current().find_wt_window(&title) {
                    crate::platform::current().apply_wt_hosted_style(hwnd);
                    crate::platform::current().set_wt_alpha(hwnd, 255);
                    crate::platform::current().activate_wt(hwnd);
                    crate::platform::current().install_wt_minimize_hook(title);
                }
                // 解锁启动状态
                let mut launching = get_is_launching().lock().unwrap();
                *launching = false;
            });
        }
    }

    /// 主进程启动 1 秒后自动检测并托管已存在的 WT 窗口
    pub fn init_delayed_check(&self, config: WtConfig) {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(1));
            if let Some(hwnd) = crate::platform::current().find_wt_window(&config.title) {
                crate::platform::current().apply_wt_hosted_style(hwnd);
                crate::platform::current().install_wt_minimize_hook(config.title);
            }
        });
    }

    /// 主进程退出时清理钩子并通知关闭 WT 窗口
    pub fn shutdown(&self, title: &str) {
        crate::platform::current().shutdown_wt(title);
    }
}
