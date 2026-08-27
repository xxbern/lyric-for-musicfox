//! 歌词窗口生命周期：Slint 宿主（方案③）。
//! 声明式 UI 见 ui/lyric.slint；本模块负责事件排空、滚动推进、拖拽、穿透与置顶。

pub mod drag;
pub mod render;
pub mod render_cache;
pub mod scroll;

use crate::LyricWindow;
use slint::ComponentHandle;

// 兼容再导出：字体/显示器工具已迁移至 services（P3）。
pub use crate::services::font::{
    FALLBACK_FONT_FAMILY, font_family_installed, load_font_bytes, resolve_runtime_font,
};
pub use crate::services::monitor;

use crate::config::Config;
use crate::context::AppContext;
use crate::services::{ServiceHandles};
use crate::window::drag::DragState;
use crate::window::render_cache::RenderCache;
use crate::window::scroll::ScrollState;
use raw_window_handle::HasWindowHandle;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const STAY_ON_TOP_REFRESH_INTERVAL: Duration = Duration::from_millis(500);
const TICK: Duration = Duration::from_millis(16);

/// 指针事件边沿标志（Slint 回调写入，tick 消费后清零）
const FLAG_PRESSED: u32 = 1;
const FLAG_RELEASED: u32 = 2;

struct Runtime {
    config: Config,
    resolved_font_family: String,
    scroll: ScrollState,
    drag: DragState,
    cache: RenderCache,
    last_frame: Option<Instant>,
    hwnd: Option<crate::platform::WindowHandle>,
    last_display_check: Instant,
    last_stay_on_top_refresh: Instant,
}

type Shared<T> = Rc<RefCell<T>>;

fn resolve_initial_position(services: &ServiceHandles, mut config: Config) -> Config {
    let layout = services.monitor.enumerate_screens();
    let ppp = monitor::primary_pixels_per_point(&layout);
    let win_size = (
        (config.window.width as f32 * ppp).round() as i32,
        (config.window.height as f32 * ppp).round() as i32,
    );

    if config.window.pos_x.is_none() || config.window.pos_y.is_none() {
        let (cx, cy) = monitor::center_on_primary_monitor(&layout, win_size, ppp);
        config.window.pos_x = Some(cx);
        config.window.pos_y = Some(cy);
    }
    if let (Some(x), Some(y)) = (config.window.pos_x, config.window.pos_y) {
        let (cx, cy) = monitor::clamp_to_primary_monitor((x, y), &layout, win_size);
        if (cx, cy) != (x, y) {
            log::info!("clamped window pos from ({x}, {y}) to ({cx}, {cy})");
        }
        config.window.pos_x = Some(cx);
        config.window.pos_y = Some(cy);
    }
    config
}

fn apply_style_properties(
    app: &LyricWindow,
    cfg: &Config,
    family: &str,
    text: &str,
    is_placeholder: bool,
    playing: bool,
) {
    app.set_lyric_text(text.into());
    // 注：Slint Text 无斜体属性，font_italic 暂不支持（与旧版视觉差异点，见迁移文档）
    app.set_font_family_name(family.into());
    app.set_font_size_px(cfg.lyric_style.font_size.max(1.0).into());
    app.set_font_weight(if cfg.lyric_style.font_bold { 700 } else { 400 });
    app.set_text_color(slint::Brush::from(render::current_text_color(&cfg.lyric_style, is_placeholder, playing)));
    match cfg.lyric_style.font_outline_color.as_deref() {
        Some(c) => app.set_outline_color(slint::Brush::from(render::color_with_alpha(c, 1.0))),
        None => app.set_outline_color(slint::Brush::from(slint::Color::from_argb_encoded(0))),
    }
    app.set_outline_width_px(cfg.lyric_style.font_outline_width.max(0) as i32);
}

fn logical_window_width(app: &LyricWindow) -> f32 {
    let size = app.window().size();
    let sf = app.window().scale_factor();
    if sf > 0.0 { size.width as f32 / sf } else { size.width as f32 }
}

pub fn run(
    ctx: Arc<AppContext>,
    event_bus_rx: crossbeam_channel::Receiver<crate::event_bus::AppEvent>,
    tray_cmd_rx: crossbeam_channel::Receiver<crate::tray::TrayCmd>,
    socket: std::net::UdpSocket,
) -> Result<(), Box<dyn std::error::Error>> {
    let services = ServiceHandles::new(ctx.clone());
    let config = resolve_initial_position(&services, ctx.get_config());
    services
        .position
        .set_current_pos((config.window.pos_x.unwrap(), config.window.pos_y.unwrap()));
    ctx.update_config(config.clone());

    let app = LyricWindow::new()?;
    crate::diag::record("slint_created");

    app.window().set_size(slint::PhysicalSize::new(
        config.window.width.max(1),
        config.window.height.max(1),
    ));
    if let (Some(x), Some(y)) = (config.window.pos_x, config.window.pos_y) {
        app.window().set_position(slint::PhysicalPosition::new(x, y));
    }

    // HWND 钩子延迟到事件循环首帧再取（窗口 realize 前 window_handle 不可用）
    services.udp.start_listening(socket);

    // 启动 1 秒后自动检测托管已存在的 WT 窗口
    services.wt.init_delayed_check(config.wt.clone());

    // 字体解析（GDI 存在性检查 + 回退）；字节加载不再需要——Skia/DirectWrite 按 family 名直接取系统字体
    let resolved_font_family =
        crate::services::font::resolve_runtime_font(&config.lyric_style.font_family);

    apply_style_properties(&app, &config, &resolved_font_family, "......", true, false);

    let rt: Shared<Runtime> = Rc::new(RefCell::new(Runtime {
        config: config.clone(),
        resolved_font_family,
        scroll: ScrollState::new(),
        drag: DragState::new(),
        cache: RenderCache::new(),
        last_frame: None,
        hwnd: None,
        last_display_check: Instant::now(),
        last_stay_on_top_refresh: Instant::now(),
    }));

    let pointer_flags: Rc<Cell<u32>> = Rc::new(Cell::new(0));
    {
        let flags = pointer_flags.clone();
        app.on_pointer_pressed(move || flags.set(flags.get() | FLAG_PRESSED));
    }
    {
        let flags = pointer_flags.clone();
        app.on_pointer_released(move || flags.set(flags.get() | FLAG_RELEASED));
    }

    let weak = app.as_weak();
    let timer = slint::Timer::default();
    {
        let rt = rt.clone();
        let ctx2 = ctx.clone();
        let pf = pointer_flags.clone();
        let ev = event_bus_rx.clone();
        let tr = tray_cmd_rx.clone();
        let svcs = services.clone();
        timer.start(
            slint::TimerMode::Repeated,
            TICK,
            move || {
                let Some(app) = weak.upgrade() else { return };
                tick(&app, &rt, &ctx2, &pf, &ev, &tr, &svcs);
            },
        );
    }

    crate::diag::record("before_gui");
    app.run()?;
    drop(timer);

    on_exit(&rt, &ctx, &services);
    Ok(())
}

fn tick(
    app: &LyricWindow,
    rt: &Shared<Runtime>,
    ctx: &Arc<AppContext>,
    pointer_flags: &Rc<Cell<u32>>,
    ev_rx: &crossbeam_channel::Receiver<crate::event_bus::AppEvent>,
    tray_rx: &crossbeam_channel::Receiver<crate::tray::TrayCmd>,
    services: &ServiceHandles,
) {
    let mut r = rt.borrow_mut();

    // —— 0. HWND 钩子（窗口 realize 后才可取；一次性） ——
    if r.hwnd.is_none() && cfg!(windows) {
        let raw = app
            .window()
            .window_handle()
            .window_handle()
            .map(|h| h.as_raw());
        if let Ok(raw) = raw {
            if let Some(h) = crate::platform::current().from_raw(raw) {
                services
                    .style
                    .set_click_through(h, r.config.window.locked);
                crate::platform::current().install_display_change_hook(h, ctx.clone());
                r.hwnd = Some(h);
                crate::diag::record("hwnd_hooked");
            }
        }
    }

    // —— 1. 事件排空 ——
    while let Ok(event) = ev_rx.try_recv() {
        match event {
            crate::event_bus::AppEvent::RequestRepaint => {}
            crate::event_bus::AppEvent::ConfigReloaded(cfg) => reload_config(app, &mut r, *cfg, ctx),
            crate::event_bus::AppEvent::TrayCmd(cmd) => handle_tray_cmd(cmd, ctx, services, &r),
            crate::event_bus::AppEvent::LyricStateChanged => {
                r.scroll.needs_recompute = true;
            }
        }
    }
    while let Ok(cmd) = tray_rx.try_recv() {
        handle_tray_cmd(cmd, ctx, services, &r);
    }

    // —— 2. 状态快照 ——
    let (playing, is_placeholder, raw_text) = {
        let guard = ctx.state.read().unwrap_or_else(|p| p.into_inner());
        (
            guard.playing,
            guard.current_line.is_placeholder,
            guard.current_line.text.to_string(),
        )
    };
    let text = if is_placeholder { "......" } else { raw_text.as_str() };

    let style_changed = {
        let style = r.config.lyric_style.clone();
        let family = r.resolved_font_family.clone();
        r.cache.update_key(text, &style, &family, is_placeholder, playing)
    };
    let text_width = app.get_lyric_text_width();
    let win_w = logical_window_width(app);

    if style_changed || r.scroll.needs_recompute {
        r.scroll.reset_for_font_change(text_width, win_w);
        r.scroll.needs_recompute = false;
    } else {
        r.scroll.text_width = text_width;
    }

    // —— 3. dt 与滚动推进 ——
    let now = Instant::now();
    let dt = r
        .last_frame
        .map(|t| now.saturating_duration_since(t).as_secs_f32())
        .unwrap_or(0.0)
        .max(0.0);
    r.last_frame = Some(now);
    r.scroll.update(dt, text, win_w);

    apply_style_properties(app, &r.config, &r.resolved_font_family, text, is_placeholder, playing);
    app.set_offset_x(r.scroll.offset_x.into());

    // —— 4. 显示器变化检查（5s 节流，拖拽期间不执行） ——
    let is_dragging = ctx.signals.is_dragging.load(std::sync::atomic::Ordering::Relaxed);
    if !is_dragging {
        let triggered_event = ctx
            .signals
            .display_changed
            .swap(false, std::sync::atomic::Ordering::SeqCst);
        let due = triggered_event
            || r.last_frame.is_none()
            || now.duration_since(r.last_display_check) >= Duration::from_secs(5);
        if due {
            r.last_display_check = now;
            let layout = services.monitor.enumerate_screens();
            let current_pos = services.position.get_current_pos();
            let ppp = services.monitor.primary_pixels_per_point(&layout);
            let size_phys = (
                (r.config.window.width as f32 * ppp).round() as i32,
                (r.config.window.height as f32 * ppp).round() as i32,
            );
            let clamped = services.monitor.clamp_position(current_pos, &layout, size_phys);
            if clamped != current_pos {
                if let Some(hwnd) = r.hwnd {
                    services.position.apply_window_pos(hwnd, clamped);
                } else {
                    services.position.set_current_pos(clamped);
                }
            }
        }
    }

    // —— 5. 拖拽 ——
    let flags = pointer_flags.get();
    pointer_flags.set(0);
    let left_pressed = flags & FLAG_PRESSED != 0;
    let left_released = flags & FLAG_RELEASED != 0;
    let current_physical = crate::platform::current().cursor_position();
    let current_outer = {
        let guard = ctx.state.read().unwrap_or_else(|p| p.into_inner());
        r.hwnd
            .and_then(|h| crate::platform::current().window_outer_position(h))
            .unwrap_or((guard.pos_x, guard.pos_y))
    };
    let was_dragging = ctx.signals.is_dragging.load(std::sync::atomic::Ordering::Relaxed);
    let new_pos = {
        let locked = r.config.window.locked;
        let mut guard = ctx.state.write().unwrap_or_else(|p| p.into_inner());
        drag::process_pointer_input(
            &mut r.drag,
            &mut guard,
            &ctx.signals,
            locked,
            current_physical,
            left_pressed,
            left_released,
            current_outer,
        )
    };
    if let (Some(pos), Some(hwnd)) = (new_pos, r.hwnd) {
        services.position.apply_window_pos(hwnd, pos);
        services.position.set_current_pos(pos);
    }

    let is_dragging = ctx.signals.is_dragging.load(std::sync::atomic::Ordering::Relaxed);
    if was_dragging && !is_dragging {
        if ctx.signals.dpi_recheck_pending.swap(false, std::sync::atomic::Ordering::SeqCst) {
            if let Some(_hwnd) = r.hwnd {
                #[cfg(windows)]
                {
                    use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
                    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

                    unsafe {
                        let hmon = MonitorFromWindow(
                            _hwnd,
                            MONITOR_DEFAULTTONEAREST,
                        );
                        let mut dpi_x = 0u32;
                        let mut dpi_y = 0u32;
                        if GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() && dpi_x > 0 {
                            let ppp = dpi_x as f32 / 96.0;
                            app.window().set_size(slint::PhysicalSize::new(
                                ((r.config.window.width as f32 * ppp).round() as u32).max(1),
                                ((r.config.window.height as f32 * ppp).round() as u32).max(1),
                            ));
                        }
                    }
                }
            }
        }
    }

    // —— 6. 置顶保活 ——
    if r.config.window.stay_on_top
        && now.duration_since(r.last_stay_on_top_refresh) >= STAY_ON_TOP_REFRESH_INTERVAL
    {
        if let Some(hwnd) = r.hwnd {
            services.style.enforce_always_on_top(hwnd);
        }
        r.last_stay_on_top_refresh = now;
    }
}

fn reload_config(app: &LyricWindow, r: &mut Runtime, cfg: Config, ctx: &Arc<AppContext>) {
    r.config = cfg.clone();
    r.resolved_font_family =
        crate::services::font::resolve_runtime_font(&cfg.lyric_style.font_family);
    r.scroll.needs_recompute = true;

    let (playing, is_placeholder, raw_text) = {
        let guard = ctx.state.read().unwrap_or_else(|p| p.into_inner());
        (
            guard.playing,
            guard.current_line.is_placeholder,
            guard.current_line.text.to_string(),
        )
    };
    let text = if is_placeholder { "......" } else { raw_text.as_str() };

    apply_style_properties(
        app,
        &cfg,
        &r.resolved_font_family,
        text,
        is_placeholder,
        playing,
    );

    let ppp = app.window().scale_factor();
    app.window().set_size(slint::PhysicalSize::new(
        ((cfg.window.width as f32 * ppp).round() as u32).max(1),
        ((cfg.window.height as f32 * ppp).round() as u32).max(1),
    ));

    if let Some(hwnd) = r.hwnd {
        crate::platform::current().apply_locked_style(hwnd, cfg.window.locked);
        if let (Some(x), Some(y)) = (cfg.window.pos_x, cfg.window.pos_y) {
            crate::platform::current().apply_outer_position(hwnd, (x, y));
        }
    }
}

fn handle_tray_cmd(
    cmd: crate::tray::TrayCmd,
    ctx: &Arc<AppContext>,
    services: &ServiceHandles,
    r: &Runtime,
) {
    #[cfg(not(windows))]
    let _ = r;

    match cmd {
        crate::tray::TrayCmd::OpenSettings => {
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe).arg("--settings").spawn();
            }
        }
        crate::tray::TrayCmd::ReloadLyrics => {
            match crate::services::config::ConfigService::load_or_default() {
                Ok(cfg) => ctx.update_config(cfg), // 经 EventBus 广播 ConfigReloaded，下一 tick 生效
                Err(e) => log::warn!("reload lyrics failed: {e}"),
            }
        }
        crate::tray::TrayCmd::ToggleWt => {
            services.wt.toggle(&ctx.get_config().wt);
        }
        crate::tray::TrayCmd::ShowContextMenu => {
            #[cfg(windows)]
            {
                let hwnd = r.hwnd.unwrap_or(windows::Win32::Foundation::HWND(std::ptr::null_mut()));
                let (tx, rx) = crossbeam_channel::unbounded();
                crate::platform::windows::show_tray_popup_menu_for_window(hwnd, &tx);
                while let Ok(sub_cmd) = rx.try_recv() {
                    handle_tray_cmd(sub_cmd, ctx, services, r);
                }
            }
        }
        crate::tray::TrayCmd::Quit => {
            // 1. 关闭 go-musicfox WT 终端窗口与事件钩子
            services.wt.shutdown(&ctx.get_config().wt.title);

            // 2. 关闭设置窗口（如果正在运行）
            #[cfg(windows)]
            {
                use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_CLOSE};
                use windows::Win32::Foundation::{WPARAM, LPARAM};
                let title_w: Vec<u16> = "lyric-for-musicfox - 设置\0".encode_utf16().collect();
                if let Ok(hwnd) = unsafe { FindWindowW(None, windows::core::PCWSTR(title_w.as_ptr())) } {
                    if !hwnd.is_invalid() {
                        unsafe {
                            let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
                        }
                    }
                }
            }

            // 3. 退出主歌词窗口事件循环
            let _ = slint::quit_event_loop();
        }
    }
}

fn on_exit(rt: &Shared<Runtime>, ctx: &Arc<AppContext>, _services: &ServiceHandles) {
    let mut r = rt.borrow_mut();
    let outer = {
        let guard = ctx.state.read().unwrap_or_else(|p| p.into_inner());
        r.hwnd
            .and_then(|h| crate::platform::current().window_outer_position(h))
            .unwrap_or((guard.pos_x, guard.pos_y))
    };
    let mut guard = ctx.state.write().unwrap_or_else(|p| p.into_inner());
    drag::flush_on_exit(&mut r.drag, &mut guard, &ctx.signals, outer);
}
