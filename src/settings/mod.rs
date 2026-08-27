//! 设置进程入口（方案③ Slint material 风格 GUI + Config 读写 + 私有一次性 pos 查询）

pub mod form;
pub mod validate;

use crate::error::{AppError, AppResult};
use crate::instance::{acquire_settings_mutex, release_settings_mutex};
use crate::path;
use crate::settings::form::{BootstrapOutcome, FormState};
use slint::{Model, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use crate::SettingsWindow as SettingsWindowUi;

fn io_err<E: std::fmt::Display>(e: E) -> AppError {
    AppError::Io(std::io::Error::other(e.to_string()))
}

pub const SETTINGS_TITLE: &str = "lyric-for-musicfox - 设置";
const POS_QUERY_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosQueryResult {
    Live(Option<i32>, Option<i32>),
    Stale,
}

/// 解析 pos 管道应答。`100,200\n` / `EMPTY,EMPTY\n`。
pub fn parse_pos_response(raw: &str) -> Result<(Option<i32>, Option<i32>), AppError> {
    use crate::protocol::IpcMessage;
    if let Ok(IpcMessage::PositionResponse(val)) = IpcMessage::parse(raw.as_bytes()) {
        match val {
            Some((x, y)) => return Ok((Some(x), Some(y))),
            None => return Ok((None, None)),
        }
    }
    let line = raw.trim_end_matches(['\r', '\n']).trim();
    let to_parse = if line.starts_with("POS ") {
        &line[4..]
    } else {
        line
    };
    if to_parse.eq_ignore_ascii_case("EMPTY,EMPTY") || to_parse.eq_ignore_ascii_case("EMPTY") {
        return Ok((None, None));
    }
    let mut parts = to_parse.split(',');
    let x = parts.next().ok_or(AppError::PosPipeTimeout)?;
    let y = parts.next().ok_or(AppError::PosPipeTimeout)?;
    if parts.next().is_some() {
        return Err(AppError::PosPipeTimeout);
    }
    let x = parse_pos_token(x)?;
    let y = parse_pos_token(y)?;
    Ok((x, y))
}

fn parse_pos_token(s: &str) -> Result<Option<i32>, AppError> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("EMPTY") || s.is_empty() {
        Ok(None)
    } else {
        s.parse::<i32>()
            .map(Some)
            .map_err(|_| AppError::PosPipeTimeout)
    }
}

fn spawn_pos_query() -> Receiver<PosQueryResult> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = match query_pos(POS_QUERY_TIMEOUT) {
            Ok((x, y)) => PosQueryResult::Live(x, y),
            Err(_) => PosQueryResult::Stale,
        };
        let _ = tx.send(result);
    });
    rx
}

fn spawn_continuous_pos_watcher() -> Receiver<PosQueryResult> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(200));
            let result = match query_pos(Duration::from_millis(150)) {
                Ok((x, y)) => PosQueryResult::Live(x, y),
                Err(_) => PosQueryResult::Stale,
            };
            if tx.send(result).is_err() {
                break;
            }
        }
    });
    rx
}

fn query_pos(timeout: Duration) -> Result<(Option<i32>, Option<i32>), AppError> {
    use crate::protocol::{platform_pipe_path, get_session_id, IpcMessage};
    let sid = get_session_id();
    let name = platform_pipe_path("pos", sid);
    let req = IpcMessage::GetPosition.to_bytes();
    if let Ok(resp) = crate::platform::current().send(&name, &req, timeout.as_millis() as u32) {
        let resp_str = String::from_utf8_lossy(&resp);
        return parse_pos_response(&resp_str);
    }
    Err(AppError::PosPipeTimeout)
}

fn opt_i32_to_text(v: Option<i32>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

fn parse_opt_i32(s: &str) -> Option<i32> {
    let s = s.trim();
    if s.is_empty() { None } else { s.parse().ok() }
}

fn open_config_folder() {
    if let Ok(dir) = crate::path::app_data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        crate::platform::current().reveal_in_file_manager(&dir);
    }
}

fn parse_color_or_default(s: &str, default: (u8, u8, u8)) -> slint::Brush {
    if let Some((r, g, b)) = crate::window::render::parse_rrggbb(s) {
        slint::Brush::from(slint::Color::from_argb_encoded(
            (0xff << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32,
        ))
    } else {
        slint::Brush::from(slint::Color::from_argb_encoded(
            (0xff << 24) | ((default.0 as u32) << 16) | ((default.1 as u32) << 8) | default.2 as u32,
        ))
    }
}

fn update_preview_brushes(app: &SettingsWindowUi) {
    let text_color = parse_color_or_default(&app.get_font_color_text(), (255, 255, 255));
    let outline_color = parse_color_or_default(&app.get_outline_color_text(), (0, 0, 0));
    app.set_preview_text_color(text_color);
    app.set_preview_outline_color(outline_color);
    // 预览描边宽度：根据 enable 标志 + 文本值同步给 UI；不合法时回退 0
    let w = parse_digits_u32(&app.get_outline_width_text());
    app.set_preview_outline_width(w.min(16) as i32);
}

// —— UI ↔ FormState 双向同步 ——

fn sync_to_ui(app: &SettingsWindowUi, form: &FormState, pos_stale: bool, families: &[String]) {
    app.set_win_width_text(form.draft.window.width.to_string().into());
    app.set_win_height_text(form.draft.window.height.to_string().into());
    app.set_pos_x_text(opt_i32_to_text(form.draft.window.pos_x).into());
    app.set_pos_y_text(opt_i32_to_text(form.draft.window.pos_y).into());
    app.set_stay_on_top(form.draft.window.stay_on_top);
    app.set_locked(form.draft.window.locked);
    app.set_font_size_pt(form.draft.lyric_style.font_size);
    app.set_font_bold(form.draft.lyric_style.font_bold);
    app.set_font_italic(form.draft.lyric_style.font_italic);
    app.set_font_color_text(form.draft.lyric_style.font_color.clone().into());
    app.set_outline_enabled(form.draft.lyric_style.font_outline_color.is_some());
    app.set_outline_color_text(
        form.draft
            .lyric_style
            .font_outline_color
            .clone()
            .unwrap_or_default()
            .into(),
    );
    app.set_outline_width_text(
        form.draft.lyric_style.font_outline_width.to_string().into(),
    );
    app.set_recv_port_text(form.draft.system.receive_port.to_string().into());
    app.set_send_port_text(form.draft.system.send_port.to_string().into());
    app.set_log_enabled(form.draft.system.log_enabled);

    // go-musicfox 路径与只读数据目录提示
    app.set_wt_musicfox_path_text(form.draft.wt.musicfox_path.clone().into());
    app.set_musicfox_data_dir_hint(crate::path::musicfox_data_dir_display().into());

    // 字体家族及下拉索引匹配回填
    let cur_family = &form.draft.lyric_style.font_family;
    app.set_font_current_value(cur_family.clone().into());
    if let Some(idx) = families.iter().position(|f| f == cur_family) {
        app.set_font_current_index(idx as i32);
    }

    app.set_pos_stale(pos_stale);
    update_preview_brushes(app);
}

fn parse_digits_u32(s: &str) -> u32 {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().unwrap_or(0)
}

fn sync_from_ui(app: &SettingsWindowUi, form: &mut FormState) {
    let d = &mut form.draft;
    d.window.width = parse_digits_u32(&app.get_win_width_text());
    d.window.height = parse_digits_u32(&app.get_win_height_text());
    d.window.pos_x = parse_opt_i32(&app.get_pos_x_text());
    d.window.pos_y = parse_opt_i32(&app.get_pos_y_text());
    d.window.stay_on_top = app.get_stay_on_top();
    d.window.locked = app.get_locked();
    d.lyric_style.font_size = app.get_font_size_pt();
    d.lyric_style.font_bold = app.get_font_bold();
    d.lyric_style.font_italic = app.get_font_italic();
    d.lyric_style.font_color = app.get_font_color_text().to_string();
    d.lyric_style.font_outline_color = if app.get_outline_enabled() {
        let t = app.get_outline_color_text().trim().to_string();
        Some(if t.is_empty() { "#000000".to_string() } else { t })
    } else {
        None
    };
    d.lyric_style.font_outline_width = parse_digits_u32(&app.get_outline_width_text());
    // 端口字段保持只读，不从 UI 同步写入，变更需重启主进程生效
    d.system.log_enabled = app.get_log_enabled();
    d.wt.musicfox_path = app.get_wt_musicfox_path_text().trim().to_string();
    let family = app.get_font_current_value().to_string();
    if !family.is_empty() {
        d.lyric_style.font_family = family;
    }
    form.mark_dirty();
}

slint::include_modules!();

/// 设置进程入口。返回进程退出码。
pub fn run() -> i32 {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::HiDpi::{
            SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        };
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    std::env::set_var("SLINT_STYLE", "material");
    match acquire_settings_mutex() {
        Ok(handle) => {
            crate::pipe::presence::start_server();
            let code = match run_inner() {
                Ok(()) => 0,
                Err(e) => {
                    e.print_to_stderr();
                    e.exit_code()
                }
            };
            release_settings_mutex(handle);
            code
        }
        Err(AppError::AnotherInstance) => {
            match crate::pipe::presence::try_activate_existing() {
                Ok(()) => 0,
                Err(e) => {
                    log::error!("presence activation failed: {}", e);
                    5
                }
            }
        }
        Err(e) => {
            e.print_to_stderr();
            e.exit_code()
        }
    }
}

fn run_inner() -> AppResult<()> {
    let config_path = path::config_path().map_err(AppError::from)?;
    let mut form = FormState::new(config_path)?;
    let outcome = form.bootstrap()?;

    let _logger_guard = if let Ok(log_path) = path::log_path() {
        crate::logger::init(
            &log_path,
            log::LevelFilter::Info,
            form.draft.system.log_enabled,
        )
    } else {
        crate::logger::init(
            std::path::Path::new("log.txt"),
            log::LevelFilter::Info,
            form.draft.system.log_enabled,
        )
    };

    if matches!(outcome, BootstrapOutcome::CreatedDefault) {
        log::info!("created default config.toml");
    }

    // 校验配置字体是否存在（回退告警）；预览由系统字体渲染
    let _resolved_font = crate::window::resolve_runtime_font(&form.draft.lyric_style.font_family);

    let pos_rx = spawn_pos_query();

    let app = SettingsWindowUi::new().map_err(io_err)?;

    #[cfg(windows)]
    {
        use raw_window_handle::HasWindowHandle;
        if let Ok(handle) = app.window().window_handle().window_handle() {
            if let raw_window_handle::RawWindowHandle::Win32(h) = handle.as_raw() {
                let hwnd = windows::Win32::Foundation::HWND(h.hwnd.get() as *mut _);
                crate::platform::windows::hook_settings_dpi_wnd_proc(hwnd);
            }
        }
    }

    // 启动持续位置同步监听器
    let pos_watcher_rx = spawn_continuous_pos_watcher();
    POS_WATCHER_RX.with(|c| *c.borrow_mut() = Some(pos_watcher_rx));

    // 字体列表：GDI 枚举零文件加载（P4-R 实测 +0.1MB 瞬时），此处直接就绪
    let families = crate::services::font::system_families();
    let model = Rc::new(VecModel::<SharedString>::from(
        families
            .iter()
            .map(|s| SharedString::from(s.as_str()))
            .collect::<Vec<SharedString>>(),
    ));
    app.set_font_model(ModelRc::new(model.clone()));

    sync_to_ui(&app, &form, false, &families);

    // FormState 不能直接进多个回调闭包 → 放入 Rc<RefCell>
    let form_rc: Rc<std::cell::RefCell<FormState>> = Rc::new(std::cell::RefCell::new(form));
    let pos_applied = Rc::new(std::cell::Cell::new(false));

    {
        let weak = app.as_weak();
        let fr = form_rc.clone();
        app.on_changed(move || {
            if let Some(app) = weak.upgrade() {
                if let Ok(mut f) = fr.try_borrow_mut() {
                    sync_from_ui(&app, &mut f);
                }
                update_preview_brushes(&app);
            }
        });
    }

    {
        let weak = app.as_weak();
        let fr = form_rc.clone();
        let families_clone = families.clone();
        app.on_save_clicked(move || {
            if let Some(app) = weak.upgrade() {
                if let Ok(mut f) = fr.try_borrow_mut() {
                    let f = &mut *f;
                    match f.flush_and_save_now() {
                        Ok(()) => {
                            app.set_save_error("".into());
                            f.show_toast("配置已保存，lyric 主进程将重启应用新配置");
                            sync_to_ui(&app, f, app.get_pos_stale(), &families_clone);
                        }
                        Err(errors) => {
                            let msg = errors
                                .iter()
                                .map(|(k, v)| format!("{k}: {v}"))
                                .collect::<Vec<_>>()
                                .join("；");
                            app.set_save_error(format!("保存失败：{msg}").into());
                        }
                    }
                }
            }
        });
    }

    app.on_open_config_clicked(open_config_folder);

    {
        let weak = app.as_weak();
        let fr = form_rc.clone();
        let families_clone = families.clone();
        app.on_browse_musicfox_clicked(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Executable", &["exe"])
                .pick_file()
            {
                if let (Some(app), Ok(mut f)) = (weak.upgrade(), fr.try_borrow_mut()) {
                    let f = &mut *f;
                    let path_str = path.to_string_lossy().to_string();
                    f.draft.wt.musicfox_path = path_str.clone();
                    app.set_wt_musicfox_path_text(path_str.into());
                    f.mark_dirty();
                    sync_to_ui(&app, f, app.get_pos_stale(), &families_clone);
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let fr = form_rc.clone();
        app.on_open_musicfox_data_clicked(move || {
            if let Some(dir) = crate::path::resolve_musicfox_data_dir() {
                let _ = std::fs::create_dir_all(&dir);
                crate::platform::current().reveal_in_file_manager(&dir);
            } else {
                if let (Some(app), Ok(mut f)) = (weak.upgrade(), fr.try_borrow_mut()) {
                    f.show_toast("无法定位 go-musicfox 数据目录");
                    let _ = app;
                }
            }
        });
    }

    {
        let fr = form_rc.clone();
        let weak = app.as_weak();
        let model_clone = model.clone();
        app.on_font_selected(move |idx| {
            if let Some(app) = weak.upgrade() {
                if let Ok(mut f) = fr.try_borrow_mut() {
                    let f = &mut *f;
                    if let Some(name) = model_clone.row_data(idx as usize) {
                        f.draft.lyric_style.font_family = name.to_string();
                        app.set_font_current_index(idx);
                        app.set_font_current_value(name);
                        f.mark_dirty();
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let fr = form_rc.clone();
        let families_clone = families.clone();
        app.on_parse_reset_clicked(move || {
            if let (Some(app), Ok(mut f)) = (weak.upgrade(), fr.try_borrow_mut()) {
                let f = &mut *f;
                match f.reset_to_default_with_backup() {
                    Ok(()) => {
                        app.set_show_parse_modal(false);
                        app.set_pos_x_text("".into());
                        app.set_pos_y_text("".into());
                        sync_to_ui(&app, f, false, &families_clone);
                    }
                    Err(e) => {
                        app.set_parse_error(e.to_string().into());
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_parse_cancel_clicked(move || {
            if let Some(app) = weak.upgrade() {
                let _ = app.window().hide();
            }
        });
    }

    {
        let weak = app.as_weak();
        let fr = form_rc.clone();
        app.on_close_save_clicked(move || {
            if let (Some(app), Ok(mut f)) = (weak.upgrade(), fr.try_borrow_mut()) {
                let f = &mut *f;
                match f.flush_and_save_now() {
                    Ok(()) => {
                        let _ = app.window().hide();
                    }
                    Err(errors) => {
                        let msg = errors
                            .iter()
                            .map(|(k, v)| format!("{k}: {v}"))
                            .collect::<Vec<_>>()
                            .join("；");
                        app.set_save_error(format!("保存失败：{msg}").into());
                        app.set_show_close_modal(false);
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let fr = form_rc.clone();
        app.on_close_discard_clicked(move || {
            if let (Some(app), Ok(mut f)) = (weak.upgrade(), fr.try_borrow_mut()) {
                { let f0 = &mut *f; let _ = f0.discard_tmp(); }
                let _ = app.window().hide();
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_close_cancel_clicked(move || {
            if let Some(app) = weak.upgrade() {
                app.set_show_close_modal(false);
            }
        });
    }

    // 关闭拦截：有未保存修改时弹确认
    {
        let weak = app.as_weak();
        let fr = form_rc.clone();
        app.window().on_close_requested(move || {
            if let Some(app) = weak.upgrade() {
                if let Ok(f) = fr.try_borrow() {
                    if f.dirty {
                        app.set_show_close_modal(true);
                        return slint::CloseRequestResponse::KeepWindowShown;
                    }
                }
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    // —— 100ms 心跳：pos 应答消费 / 防抖写 tmp / toast 到期 ——
    let timer = slint::Timer::default();
    {
        let weak = app.as_weak();
        let fr = form_rc.clone();
        let pa = pos_applied.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(100),
            move || {
                let Some(app) = weak.upgrade() else { return };
                if let Ok(mut f) = fr.try_borrow_mut() {
                    let f = &mut *f;

                    // pos 查询应答（一次性）
                    if !pa.get() {
                        if let Some(rx) = POS_RX.with(|c| c.borrow_mut().take()) {
                            match rx.try_recv() {
                                Ok(PosQueryResult::Live(x, y)) => {
                                    f.draft.window.pos_x = x;
                                    f.draft.window.pos_y = y;
                                    app.set_pos_x_text(opt_i32_to_text(x).into());
                                    app.set_pos_y_text(opt_i32_to_text(y).into());
                                    app.set_pos_stale(false);
                                    pa.set(true);
                                }
                                Ok(PosQueryResult::Stale)
                                | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    app.set_pos_stale(true);
                                    pa.set(true);
                                }
                                Err(std::sync::mpsc::TryRecvError::Empty) => {
                                    POS_RX.with(|c| *c.borrow_mut() = Some(rx));
                                }
                            }
                        } else {
                            pa.set(true);
                        }
                    }

                    // 持续位置同步：歌词窗口在任意屏幕被拖动时即刻刷新 X / Y
                    POS_WATCHER_RX.with(|c| {
                        if let Some(rx) = c.borrow().as_ref() {
                            while let Ok(res) = rx.try_recv() {
                                match res {
                                    PosQueryResult::Live(x, y) => {
                                        if f.draft.window.pos_x != x || f.draft.window.pos_y != y {
                                            f.draft.window.pos_x = x;
                                            f.draft.window.pos_y = y;
                                            app.set_pos_x_text(opt_i32_to_text(x).into());
                                            app.set_pos_y_text(opt_i32_to_text(y).into());
                                            app.set_pos_stale(false);
                                        }
                                    }
                                    PosQueryResult::Stale => {
                                        // 主进程未运行或已退出
                                    }
                                }
                            }
                        }
                    });

                    let _ = f.flush_tmp_if_due();

                    // toast 同步：FormState 到期即隐藏
                    match f.toast_visible() {
                        Some(t) => {
                            app.set_toast_text(t.into());
                            app.set_show_toast(true);
                        }
                        None => app.set_show_toast(false),
                    }
                }
            },
        );
    }

    // 把 pos_rx 存入 thread_local 供心跳取用
    POS_RX.with(|c| *c.borrow_mut() = Some(pos_rx));

    // 解析失败模态初始态
    if let Some(m) = form_rc.borrow().parse_modal.as_ref() {
        app.set_parse_error(m.error.clone().into());
        app.set_show_parse_modal(true);
    }

    let res = app.run();

    drop(timer);
    res.map_err(io_err)
}

thread_local! {
    static POS_RX: std::cell::RefCell<Option<Receiver<PosQueryResult>>> =
        const { std::cell::RefCell::new(None) };
    static POS_WATCHER_RX: std::cell::RefCell<Option<Receiver<PosQueryResult>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_live_coordinates() {
        assert_eq!(
            parse_pos_response("100,200\n").unwrap(),
            (Some(100), Some(200))
        );
        assert_eq!(parse_pos_response("EMPTY,EMPTY\n").unwrap(), (None, None));
        assert_eq!(parse_pos_response("-12,34").unwrap(), (Some(-12), Some(34)));
    }

    #[test]
    fn parse_malformed_rejected() {
        assert!(parse_pos_response("nope").is_err());
        assert!(parse_pos_response("1,2,3\n").is_err());
        assert!(parse_pos_response("").is_err());
    }
}
