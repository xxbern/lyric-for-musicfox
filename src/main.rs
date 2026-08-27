#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! P1 入口：CLI 解析 → --settings 分发 → 主进程 Mutex → logger → config load → GUI
//!
//! 启动顺序遵循 dev.md §5.0：
//!   1. CLI 解析（无副作用）
//!   2. --settings 分支 → settings::run() → 直接返回 exit code
//!   3. 主进程 Mutex（先于 logger，避免第二实例写盘）
//!   4. logger 初始化（仅第一个实例执行到这里）
//!   5. config load
//!   6. ★ LyricApp::run(config) → Slint 事件循环
//!   7. 退出：Drop Mutex

use std::process::exit;

use clap::error::ErrorKind;
use clap::Parser;

use lyric_for_musicfox::{cli, load_config, logger, path, settings, window};

fn main() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::HiDpi::{
            SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        };
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    std::env::set_var("SLINT_STYLE", "material");

    // panic 落盘：双击启动时无控制台，崩溃信息写入 exe 工作目录 crash.log
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("crash.log")
        {
            let _ = writeln!(f, "[{:?}] panic: {info}", std::time::SystemTime::now());
        }
        eprintln!("panic: {info}");
    }));

    lyric_for_musicfox::diag::init_from_env();
    lyric_for_musicfox::diag::record("main_enter");

    // 1. CLI 解析（无副作用）
    let cli = cli::Cli::try_parse().unwrap_or_else(|e| {
        match e.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                // --help / --version 必须 exit 0（dev.md §9）
                e.print().ok();
                exit(0);
            }
            _ => {
                // 其他解析错误 → exit 4
                eprintln!("{e}");
                exit(4);
            }
        }
    });

    // 2. --settings 分支：直接转交设置进程入口
    if cli.settings_requested() {
        let code = settings::run();
        exit(code);
    }

    // 3. 主进程 Mutex（先于 logger，避免第二实例写盘）
    let _mutex_handle = match lyric_for_musicfox::platform::current().acquire_main_mutex() {
        Ok(opt) => opt,
        Err(e) => {
            if matches!(e, lyric_for_musicfox::AppError::AnotherInstance) {
                eprintln!("another instance running");
                exit(6);
            }
            eprintln!("mutex error: {}", e);
            exit(e.exit_code());
        }
    };

    // 4. 解析 config_path 与加载 config
    let config_path = match path::config_path() {
        Ok(p) => p,
        Err(e) => {
            // %APPDATA% 不可写 → exit 2（dev.md §9）
            eprintln!("config path error: {}", e);
            exit(2);
        }
    };

    let mut config_failed = false;
    let mut load_config_err = None;
    let config = match load_config(&config_path) {
        Ok(c) => c,
        Err(e) => {
            config_failed = true;
            load_config_err = Some(e);
            let mut def = lyric_for_musicfox::Config::default();
            def.system.log_enabled = false;
            def
        }
    };

    // 5. logger 初始化（仅第一个实例执行到这里）
    let default_level = if cli.benchmark {
        log::LevelFilter::Trace
    } else {
        log::LevelFilter::Info
    };
    let _logger_guard = if let Ok(log_path) = path::log_path() {
        logger::init(&log_path, default_level, config.system.log_enabled)
    } else {
        // 路径解析失败 → stderr-only fallback
        logger::init(
            std::path::Path::new("log.txt"),
            default_level,
            config.system.log_enabled,
        )
    };

    if let Some(e) = load_config_err {
        // P4+ 主进程：使用默认配置继续启动 + 托盘闪烁
        logger::error!("Failed to load config, using default: {}", e);
    }

    // 6. Bind UDP port directly (exit 1 on bind failure)
    let socket = match lyric_for_musicfox::lyric::udp::bind(config.system.receive_port) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("port {} already in use: {}", config.system.receive_port, e);
            exit(1);
        }
    };

    // 7. Initialize shared state and signals
    let (event_bus, event_bus_rx) = lyric_for_musicfox::event_bus::EventBus::new(256);
    let ctx = std::sync::Arc::new(lyric_for_musicfox::context::AppContext::new(
        config,
        lyric_for_musicfox::lyric::state::LyricState::placeholder(),
        event_bus,
    ));

    // P4: Named pipes reload and pos coordinate servers
    lyric_for_musicfox::pipe::reload::start_server(config_path, ctx.clone());
    lyric_for_musicfox::pipe::pos::start_server(ctx.clone());

    // P4: Spawn the System Tray and command routing
    let (tray_handle, tray_cmd_rx) = lyric_for_musicfox::tray::init_tray(ctx.event_bus.clone());
    if config_failed {
        lyric_for_musicfox::tray::update_tray_flash(&tray_handle, true);
    }

    let current_config = ctx.get_config();
    logger::info!(
        "lyric-for-musicfox starting (P4 GUI/Tray/IPC); window {}x{}, font={}",
        current_config.window.width,
        current_config.window.height,
        current_config.lyric_style.font_family
    );

    lyric_for_musicfox::diag::record("before_gui");
    lyric_for_musicfox::diag::spawn_sampler(5);

    // 9. P4: run window with config, state, signals, new_config, tray_cmd_rx, socket
    let run_res = window::run(ctx, event_bus_rx, tray_cmd_rx, socket);
    if let Err(e) = run_res {
        logger::error!("slint error: {e}");
        drop(_logger_guard);
        exit(1);
    }

    logger::info!("exiting");
}
