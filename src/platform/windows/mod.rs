use std::cell::RefCell;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED,
    HANDLE, INVALID_HANDLE_VALUE, HWND, POINT, RECT, LRESULT, WPARAM, LPARAM, COLORREF,
    GENERIC_READ, GENERIC_WRITE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX, PIPE_ACCESS_INBOUND,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW,
    PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
use windows::Win32::UI::Accessibility::{
    SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, BringWindowToTop, FindWindowW, GetCursorPos, GetWindowLongPtrW,
    GetWindowRect, GetWindowThreadProcessId, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, CallWindowProcW, DefWindowProcW, GWL_EXSTYLE, GWL_STYLE, HWND_TOP, HWND_TOPMOST, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_EX_LAYERED, WS_CAPTION, WS_SYSMENU, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    GetSystemMenu, EnableMenuItem, SC_CLOSE, MF_GRAYED, MF_BYCOMMAND, SetLayeredWindowAttributes,
    GetLayeredWindowAttributes, LWA_ALPHA, LAYERED_WINDOW_ATTRIBUTES_FLAGS, IsWindow, GetWindowTextW,
    EnumWindows, PostMessageW, WM_CLOSE, SW_RESTORE, SW_SHOWNORMAL, EVENT_SYSTEM_MINIMIZESTART,
    WM_NULL, CreatePopupMenu, AppendMenuW, TrackPopupMenuEx, DestroyMenu,
    MF_STRING, MF_SEPARATOR, TPM_TOPALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WNDPROC,
    GetSystemMetrics, SM_CYMENU,
};

use crate::context::AppContext;
use crate::error::AppError;
use crate::tray::{TrayCmd, TrayHandle};
use crate::protocol::{IpcMessage, platform_pipe_path, get_session_id};
use super::r#trait::*;

pub struct WindowsBackend;

impl Platform for WindowsBackend {}

impl PlatformSessionId for WindowsBackend {
    fn session_id(&self) -> u32 {
        unsafe {
            let pid = GetCurrentProcessId();
            let mut session_id = 0;
            if ProcessIdToSessionId(pid, &mut session_id).is_ok() {
                session_id
            } else {
                0
            }
        }
    }
}

fn acquire_named_mutex(name: &str) -> Result<Option<HANDLE>, AppError> {
    let name_w: Vec<u16> = format!("{}\0", name).encode_utf16().collect();
    let handle = unsafe {
        CreateMutexW(
            None,
            false,
            windows::core::PCWSTR(name_w.as_ptr()),
        )
    };
    match handle {
        Ok(h) => {
            let err = unsafe { GetLastError() };
            if err == ERROR_ALREADY_EXISTS {
                unsafe {
                    let _ = CloseHandle(h);
                }
                Err(AppError::AnotherInstance)
            } else {
                Ok(Some(h))
            }
        }
        Err(e) => Err(AppError::MutexCreate(format!("CreateMutexW failed: {e:?}"))),
    }
}

impl PlatformInstance for WindowsBackend {
    fn acquire_main_mutex(&self) -> Result<Option<super::MutexHandle>, AppError> {
        acquire_named_mutex("Local\\lyric-for-musicfox-main")
    }

    fn acquire_settings_mutex(&self) -> Result<Option<super::MutexHandle>, AppError> {
        acquire_named_mutex("Local\\lyric-for-musicfox-settings")
    }

    fn release_mutex(&self, h: Option<super::MutexHandle>) {
        if let Some(h) = h {
            unsafe {
                let _ = CloseHandle(h);
            }
        }
    }

    fn probe_port(&self, port: u16) -> Result<(), AppError> {
        use std::net::UdpSocket;
        match UdpSocket::bind(("127.0.0.1", port)) {
            Ok(_) => Ok(()),
            Err(_e) => Err(AppError::PortInUse(port)),
        }
    }
}

impl PlatformTray for WindowsBackend {
    fn init(&self, event_bus: crate::event_bus::EventBus) -> Result<(Option<TrayHandle>, crossbeam_channel::Receiver<TrayCmd>), String> {
        let icon_normal = tray_icon::Icon::from_resource(1, None)
            .unwrap_or_else(|e| {
                log::warn!("Failed to load normal tray icon: {e:?}, falling back to default blue icon");
                let mut rgba = Vec::new();
                for _ in 0..(16 * 16) {
                    rgba.extend_from_slice(&[0, 120, 215, 255]);
                }
                tray_icon::Icon::from_rgba(rgba, 16, 16).unwrap()
            });
        let icon_normal = Rc::new(icon_normal);

        let icon_gray = tray_icon::Icon::from_resource(1, None)
            .unwrap_or_else(|e| {
                log::warn!("Failed to load gray tray icon: {e:?}, falling back to default gray icon");
                let mut rgba = Vec::new();
                for _ in 0..(16 * 16) {
                    rgba.extend_from_slice(&[128, 128, 128, 128]);
                }
                tray_icon::Icon::from_rgba(rgba, 16, 16).unwrap()
            });
        let icon_gray = Rc::new(icon_gray);

        // 不挂载 with_menu，确保左键点击绝对不会激活弹出菜单
        let tray = tray_icon::TrayIconBuilder::new()
            .with_tooltip("lyric-for-musicfox")
            .with_icon((*icon_normal).clone())
            .build()
            .ok();

        let tray_arc = Arc::new(Mutex::new(tray));
        let flashing = Arc::new(Mutex::new(false));

        let (tx, rx) = crossbeam_channel::unbounded();
        let tx_clone = tx.clone();

        thread::spawn(move || {
            let tray_channel = tray_icon::TrayIconEvent::receiver();
            loop {
                let mut sent = false;
                while let Ok(event) = tray_channel.try_recv() {
                    match event {
                        tray_icon::TrayIconEvent::Click { button, .. }
                        | tray_icon::TrayIconEvent::DoubleClick { button, .. } => {
                            match button {
                                tray_icon::MouseButton::Left => {
                                    let _ = tx_clone.send(TrayCmd::ToggleWt);
                                    sent = true;
                                }
                                tray_icon::MouseButton::Right => {
                                    // 防抖：菜单关闭后 250ms 内的右键事件认为是 tray-icon 误报
                                    if should_suppress_tray_menu() {
                                        log::debug!("tray right-click suppressed by debounce");
                                    } else {
                                        let _ = tx_clone.send(TrayCmd::ShowContextMenu);
                                        sent = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
                if sent {
                    event_bus.emit(crate::event_bus::AppEvent::RequestRepaint);
                }
                thread::sleep(Duration::from_millis(30));
            }
        });

        let handle = TrayHandle {
            icon_normal,
            icon_gray,
            tray: tray_arc,
            flashing,
        };

        Ok((Some(handle), rx))
    }

    fn update_flash(&self, handle: &Option<TrayHandle>, flash: bool) {
        if let Some(h) = handle {
            let mut flashing = h.flashing.lock().unwrap();
            *flashing = flash;
        }
    }
}

pub fn show_tray_popup_menu_for_window(hwnd: HWND, tx: &crossbeam_channel::Sender<TrayCmd>) {
    // 记录托盘菜单弹出时间，用于防抖 tray-icon 菜单关闭瞬间的误报 Click 事件
    mark_tray_menu_shown();

    unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

        let hmenu = match CreatePopupMenu() {
            Ok(h) => h,
            Err(_) => return,
        };
        if hmenu.is_invalid() {
            return;
        }

        const ID_CONFIG: usize = 1001;
        const ID_RELOAD: usize = 1002;
        const ID_QUIT: usize = 1003;

        let config_w = to_wstring("配置");
        let reload_w = to_wstring("重载歌词");
        let quit_w = to_wstring("退出");

        let _ = AppendMenuW(hmenu, MF_STRING, ID_CONFIG, windows::core::PCWSTR(config_w.as_ptr()));
        let _ = AppendMenuW(hmenu, MF_STRING, ID_RELOAD, windows::core::PCWSTR(reload_w.as_ptr()));
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
        let _ = AppendMenuW(hmenu, MF_STRING, ID_QUIT, windows::core::PCWSTR(quit_w.as_ptr()));

        // MSDN Q135788 规范：激活所属窗口 -> 弹出菜单 -> 发送 WM_NULL 确保焦点释放
        let target_hwnd = if !hwnd.is_invalid() && IsWindow(hwnd).as_bool() {
            hwnd
        } else {
            FindWindowW(None, windows::core::w!("lyric-for-musicfox")).unwrap_or_default()
        };

        if !target_hwnd.is_invalid() {
            let _ = SetForegroundWindow(target_hwnd);
        }

        // 安全弹出：菜单底部不能超出鼠标所在屏幕的工作区底边
        let safe_y = clamp_popup_y_to_work_area(pt);

        let cmd_id = TrackPopupMenuEx(
            hmenu,
            (TPM_TOPALIGN | TPM_LEFTALIGN | TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
            pt.x,
            safe_y,
            target_hwnd,
            None,
        );

        if !target_hwnd.is_invalid() {
            let _ = PostMessageW(target_hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        }
        let _ = DestroyMenu(hmenu);

        match cmd_id.0 as usize {
            ID_CONFIG => {
                let _ = tx.send(TrayCmd::OpenSettings);
            }
            ID_RELOAD => {
                let _ = tx.send(TrayCmd::ReloadLyrics);
            }
            ID_QUIT => {
                let _ = tx.send(TrayCmd::Quit);
            }
            _ => {}
        }
    }
}

fn to_wstring(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// 计算托盘菜单安全的弹出 Y 坐标。
/// 约束：菜单顶部 ≤ 鼠标 Y（不变），菜单底部 ≤ 鼠标所在屏幕工作区底边。
/// 菜单项数估算：调用方传入 hardcoded item count（本函数里硬编码为常量）。
///
/// 步骤：
///   1. `MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST)` 拿到鼠标所在屏幕；
///   2. `GetMonitorInfoW(hmon, &mut info)` 拿 `rcWork`（刨去任务栏的工作区）；
///   3. 用 `GetSystemMetrics(SM_CYMENU)` 作为单行高度 * 项数 + 边框 ≈ 菜单高度；
///   4. 若 `pt.y + menu_height > rcWork.bottom`，y = rcWork.bottom - menu_height（不超过工作区上边）。
unsafe fn clamp_popup_y_to_work_area(pt: POINT) -> i32 {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};

    const MENU_ITEMS: i32 = 4; // 配置 / 重载歌词 / 分隔 / 退出 = 4 行
    let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    if hmon.is_invalid() {
        return pt.y;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(hmon, &mut info).as_bool() {
        return pt.y;
    }
    let row_h = GetSystemMetrics(SM_CYMENU).max(1) as i32;
    let menu_h = row_h * MENU_ITEMS + 4; // +边框
    let max_y = info.rcWork.bottom - menu_h;
    let min_y = info.rcWork.top;
    pt.y.clamp(min_y, max_y)
}

fn connect_pipe(name: &str, timeout_ms: u32) -> Result<File, String> {
    let name_w = to_wstring(name);
    let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
    loop {
        let handle = unsafe {
            CreateFileW(
                windows::core::PCWSTR(name_w.as_ptr()),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };
        match handle {
            Ok(h) if h != INVALID_HANDLE_VALUE => {
                let file = unsafe { File::from_raw_handle(h.0 as *mut _) };
                return Ok(file);
            }
            _ => {
                let err = unsafe { GetLastError() };
                if err == ERROR_PIPE_BUSY || err == ERROR_FILE_NOT_FOUND {
                    let now = Instant::now();
                    if now >= deadline {
                        return Err(format!("pipe connection timeout: {}", name));
                    }
                    let remaining = deadline.saturating_duration_since(now).as_millis() as u32;
                    unsafe {
                        let _ = WaitNamedPipeW(
                            windows::core::PCWSTR(name_w.as_ptr()),
                            remaining.min(100),
                        );
                    }
                } else {
                    return Err(format!(
                        "pipe connection failed: {:?}, error: {:?}",
                        name, err
                    ));
                }
            }
        }
    }
}

impl PlatformPipeServer for WindowsBackend {
    fn start_reload(&self, ctx: Arc<AppContext>, config_path: PathBuf) -> Result<(), String> {
        thread::spawn(move || {
            let sid = get_session_id();
            let name = platform_pipe_path("reload", sid);
            let name_w = to_wstring(&name);
            loop {
                let handle = unsafe {
                    CreateNamedPipeW(
                        windows::core::PCWSTR(name_w.as_ptr()),
                        PIPE_ACCESS_INBOUND,
                        windows::Win32::System::Pipes::NAMED_PIPE_MODE(
                            PIPE_TYPE_BYTE.0 | PIPE_READMODE_BYTE.0 | PIPE_WAIT.0,
                        ),
                        PIPE_UNLIMITED_INSTANCES,
                        512,
                        512,
                        0,
                        None,
                    )
                };
                let h = handle;
                if h != INVALID_HANDLE_VALUE {
                    let connected = unsafe { ConnectNamedPipe(h, None).is_ok() };
                    let err = unsafe { GetLastError() };
                    if connected || err == ERROR_PIPE_CONNECTED {
                        let file = unsafe { File::from_raw_handle(h.0 as *mut _) };
                        let mut file = std::mem::ManuallyDrop::new(file);
                        let mut reader = BufReader::new(&mut *file);
                        let mut line = String::new();
                        if reader.read_line(&mut line).is_ok() {
                            if let Ok(IpcMessage::ReloadConfig) = IpcMessage::parse(line.trim_end().as_bytes()) {
                                if let Ok(cfg) = crate::load_config(&config_path) {
                                    ctx.update_config(cfg);
                                }
                            }
                        }
                    }
                    unsafe {
                        let _ = DisconnectNamedPipe(h);
                        let _ = CloseHandle(h);
                    }
                } else {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
        Ok(())
    }

    fn start_pos(&self, ctx: Arc<AppContext>) -> Result<(), String> {
        thread::spawn(move || {
            let sid = get_session_id();
            let name = platform_pipe_path("pos", sid);
            let name_w = to_wstring(&name);
            loop {
                let handle = unsafe {
                    CreateNamedPipeW(
                        windows::core::PCWSTR(name_w.as_ptr()),
                        PIPE_ACCESS_DUPLEX,
                        windows::Win32::System::Pipes::NAMED_PIPE_MODE(
                            PIPE_TYPE_BYTE.0 | PIPE_READMODE_BYTE.0 | PIPE_WAIT.0,
                        ),
                        PIPE_UNLIMITED_INSTANCES,
                        512,
                        512,
                        0,
                        None,
                    )
                };
                let h = handle;
                if h != INVALID_HANDLE_VALUE {
                    let connected = unsafe { ConnectNamedPipe(h, None).is_ok() };
                    let err = unsafe { GetLastError() };
                    if connected || err == ERROR_PIPE_CONNECTED {
                        let file = unsafe { File::from_raw_handle(h.0 as *mut _) };
                        let mut file = std::mem::ManuallyDrop::new(file);
                        let mut reader = BufReader::new(&mut *file);
                        let mut line = String::new();
                        if reader.read_line(&mut line).is_ok() {
                            if let Ok(IpcMessage::GetPosition) = IpcMessage::parse(line.trim_end().as_bytes()) {
                                let (pos_x, pos_y) = {
                                    let s = ctx.state.read().unwrap_or_else(|p| p.into_inner());
                                    (s.pos_x, s.pos_y)
                                };
                                let resp = IpcMessage::PositionResponse(Some((pos_x, pos_y))).to_bytes();
                                if let Ok(mut f) = file.try_clone() {
                                    let _ = f.write_all(&resp);
                                    let _ = f.flush();
                                }
                            }
                        }
                    }
                    unsafe {
                        let _ = DisconnectNamedPipe(h);
                        let _ = CloseHandle(h);
                    }
                } else {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
        Ok(())
    }

    fn start_presence(&self) -> Result<(), String> {
        thread::spawn(move || {
            let sid = get_session_id();
            let name = platform_pipe_path("presence", sid);
            let name_w = to_wstring(&name);
            loop {
                let handle = unsafe {
                    CreateNamedPipeW(
                        windows::core::PCWSTR(name_w.as_ptr()),
                        PIPE_ACCESS_DUPLEX,
                        windows::Win32::System::Pipes::NAMED_PIPE_MODE(
                            PIPE_TYPE_BYTE.0 | PIPE_READMODE_BYTE.0 | PIPE_WAIT.0,
                        ),
                        PIPE_UNLIMITED_INSTANCES,
                        512,
                        512,
                        0,
                        None,
                    )
                };
                let h = handle;
                if h != INVALID_HANDLE_VALUE {
                    let connected = unsafe { ConnectNamedPipe(h, None).is_ok() };
                    let err = unsafe { GetLastError() };
                    if connected || err == ERROR_PIPE_CONNECTED {
                        let file = unsafe { File::from_raw_handle(h.0 as *mut _) };
                        let mut file = std::mem::ManuallyDrop::new(file);
                        let mut reader = BufReader::new(&mut *file);
                        let mut line = String::new();
                        if reader.read_line(&mut line).is_ok() {
                            if let Ok(IpcMessage::ActivateSettings(_pid)) = IpcMessage::parse(line.trim_end().as_bytes()) {
                                let hwnd_res = unsafe {
                                    FindWindowW(None, windows::core::w!("lyric-for-musicfox - 设置"))
                                };
                                if let Ok(hwnd) = hwnd_res {
                                    if hwnd.0 != std::ptr::null_mut() {
                                        unsafe {
                                            let _ = ShowWindow(hwnd, SW_RESTORE);
                                            let _ = BringWindowToTop(hwnd);
                                            let _ = SetForegroundWindow(hwnd);
                                        }
                                    }
                                }
                                if let Ok(mut f) = file.try_clone() {
                                    let resp = IpcMessage::Success.to_bytes();
                                    let _ = f.write_all(&resp);
                                    let _ = f.flush();
                                }
                            }
                        }
                    }
                    unsafe {
                        let _ = DisconnectNamedPipe(h);
                        let _ = CloseHandle(h);
                    }
                } else {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
        Ok(())
    }
}

impl PlatformPipeClient for WindowsBackend {
    fn send(&self, pipe_name: &str, payload: &[u8], timeout_ms: u32) -> Result<Vec<u8>, String> {
        let mut file = connect_pipe(pipe_name, timeout_ms)?;
        file.write_all(payload).map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        Ok(line.into_bytes())
    }

    fn try_activate(&self, _pid: u32) -> Result<(), String> {
        let hwnd_res = unsafe { FindWindowW(None, windows::core::w!("lyric-for-musicfox - 设置")) };
        if let Ok(hwnd) = hwnd_res {
            if hwnd.0 != std::ptr::null_mut() {
                let mut old_pid = 0;
                unsafe {
                    GetWindowThreadProcessId(hwnd, Some(&mut old_pid));
                }
                if old_pid != 0 {
                    unsafe {
                        let _ = AllowSetForegroundWindow(old_pid);
                    }
                }
                let sid = get_session_id();
                let name = platform_pipe_path("presence", sid);
                let req = IpcMessage::ActivateSettings(std::process::id()).to_bytes();
                let resp = self.send(&name, &req, 2000)?;
                if let Ok(IpcMessage::Success) = IpcMessage::parse(&resp) {
                    return Ok(());
                } else {
                    return Err(format!("unexpected response: {:?}", std::str::from_utf8(&resp)));
                }
            }
        }
        Err("no window found".to_string())
    }
}

static PREV_WND_PROC: OnceLock<Mutex<Option<WNDPROC>>> = OnceLock::new();

fn set_prev_wnd_proc(proc: WNDPROC) {
    let locker = PREV_WND_PROC.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = locker.lock() {
        *guard = Some(proc);
    }
}

fn get_prev_wnd_proc() -> Option<WNDPROC> {
    PREV_WND_PROC.get().and_then(|m| m.lock().ok().and_then(|g| *g))
}

thread_local! {
    static WND_CTX: RefCell<Option<Arc<AppContext>>> = RefCell::new(None);
}

pub unsafe extern "system" fn monitor_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    use std::sync::atomic::Ordering;
    if msg == 0x007E { // WM_DISPLAYCHANGE = 0x007E
        WND_CTX.with(|cell| {
            if let Some(ctx) = &*cell.borrow() {
                ctx.signals.display_changed.store(true, Ordering::SeqCst);
            }
        });
    } else if msg == 0x02E0 { // WM_DPICHANGED = 0x02E0
        let is_dragging = (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(0x01) as i16) < 0;
        if is_dragging {
            WND_CTX.with(|cell| {
                if let Some(ctx) = &*cell.borrow() {
                    ctx.signals.dpi_recheck_pending.store(true, Ordering::SeqCst);
                }
            });
            return LRESULT(0);
        }
        let prc = lparam.0 as *const RECT;
        if !prc.is_null() {
            let r = *prc;
            let _ = SetWindowPos(
                hwnd,
                HWND_TOP,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        return LRESULT(0);
    }
    if let Some(prev) = get_prev_wnd_proc() {
        CallWindowProcW(prev, hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

static PREV_SETTINGS_WND_PROC: OnceLock<Mutex<Option<WNDPROC>>> = OnceLock::new();
static SETTINGS_DPI_PENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SETTINGS_SUGGESTED_SIZE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub unsafe extern "system" fn settings_dpi_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    use std::sync::atomic::Ordering;
    if msg == 0x02E0 { // WM_DPICHANGED = 0x02E0: 跨屏拖拽防震荡处理
        let is_dragging = (windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(0x01) as i16) < 0; // VK_LBUTTON = 0x01
        if is_dragging {
            let prc = lparam.0 as *const RECT;
            if !prc.is_null() {
                let r = *prc;
                let width = (r.right - r.left) as u32;
                let height = (r.bottom - r.top) as u32;
                let packed = ((width as u64) << 32) | (height as u64);
                SETTINGS_SUGGESTED_SIZE.store(packed, Ordering::SeqCst);
                SETTINGS_DPI_PENDING.store(true, Ordering::SeqCst);
            }
            return LRESULT(0);
        }

        let prc = lparam.0 as *const RECT;
        if !prc.is_null() {
            let r = *prc;
            let _ = SetWindowPos(
                hwnd,
                HWND_TOP,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        return LRESULT(0);
    } else if msg == 0x0232 || msg == 0x0202 { // WM_EXITSIZEMOVE = 0x0232 or WM_LBUTTONUP = 0x0202
        if SETTINGS_DPI_PENDING.swap(false, Ordering::SeqCst) {
            let packed = SETTINGS_SUGGESTED_SIZE.swap(0, Ordering::SeqCst);
            if packed > 0 {
                let width = (packed >> 32) as i32;
                let height = (packed & 0xFFFFFFFF) as i32;
                let mut rect = RECT::default();
                if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOP,
                        rect.left,
                        rect.top,
                        width,
                        height,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
        }
    }
    let prev = PREV_SETTINGS_WND_PROC.get().and_then(|m| m.lock().ok().and_then(|g| *g));
    if let Some(p) = prev {
        CallWindowProcW(p, hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

pub fn hook_settings_dpi_wnd_proc(hwnd: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::GWL_WNDPROC;
    unsafe {
        let prev = SetWindowLongPtrW(
            hwnd,
            GWL_WNDPROC,
            settings_dpi_wnd_proc as *const () as isize,
        );
        if prev != 0 {
            let prev_wndproc: WNDPROC = std::mem::transmute(prev);
            let locker = PREV_SETTINGS_WND_PROC.get_or_init(|| Mutex::new(None));
            if let Ok(mut guard) = locker.lock() {
                *guard = Some(prev_wndproc);
            }
        }
    }
}

impl PlatformShell for WindowsBackend {
    fn reveal_in_file_manager(&self, path: &std::path::Path) {
        let _ = std::process::Command::new("explorer.exe").arg(path).spawn();
    }
}

impl PlatformMonitor for WindowsBackend {
    fn enumerate(&self) -> crate::services::monitor::MonitorLayout {
        use windows::Win32::Foundation::{BOOL, LPARAM};
        use windows::Win32::Graphics::Gdi::{
            EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
        };
        use crate::services::monitor::{MonitorRect, fallback};

        const MONITORINFOF_PRIMARY: u32 = 1;

        struct Collect {
            monitors: Vec<MonitorRect>,
            primary: Option<usize>,
        }

        unsafe extern "system" fn callback(
            hmon: HMONITOR,
            _hdc: HDC,
            _rect: *mut RECT,
            data: LPARAM,
        ) -> BOOL {
            let collect = unsafe { &mut *(data.0 as *mut Collect) };
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if unsafe { GetMonitorInfoW(hmon, &mut info) }.as_bool() {
                let r = info.rcMonitor;
                let rect = MonitorRect {
                    left: r.left,
                    top: r.top,
                    right: r.right,
                    bottom: r.bottom,
                };
                let idx = collect.monitors.len();
                if info.dwFlags & MONITORINFOF_PRIMARY != 0 {
                    collect.primary = Some(idx);
                }
                collect.monitors.push(rect);
            }
            BOOL(1)
        }

        let mut collect = Collect {
            monitors: Vec::new(),
            primary: None,
        };
        let ok = unsafe {
            EnumDisplayMonitors(
                HDC::default(),
                None,
                Some(callback),
                LPARAM(&mut collect as *mut Collect as isize),
            )
        };
        if !ok.as_bool() || collect.monitors.is_empty() {
            log::warn!("EnumDisplayMonitors failed or empty; using 1920x1080 fallback layout");
            return fallback();
        }

        let primary = collect.primary.unwrap_or(0);
        let primary_monitor = collect.monitors.get(primary).copied().unwrap_or_else(|| {
            collect
                .monitors
                .first()
                .copied()
                .unwrap_or(MonitorRect { left: 0, top: 0, right: 1920, bottom: 1080 })
        });
        crate::services::monitor::MonitorLayout {
            monitors: collect.monitors,
            primary,
            primary_monitor,
        }
    }

    fn primary_pixels_per_point(&self, layout: &crate::services::monitor::MonitorLayout) -> f32 {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY};
        use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

        let origin = POINT {
            x: layout.primary_monitor.left,
            y: layout.primary_monitor.top,
        };
        let hmon = unsafe { MonitorFromPoint(origin, MONITOR_DEFAULTTOPRIMARY) };
        if hmon.is_invalid() {
            return 1.0;
        }
        let mut dpi_x = 0u32;
        let mut dpi_y = 0u32;
        if unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.is_err() {
            return 1.0;
        }
        let _ = dpi_y;
        if dpi_x == 0 {
            1.0
        } else {
            dpi_x as f32 / 96.0
        }
    }
}

impl PlatformWindowStyle for WindowsBackend {
    fn from_raw(&self, raw: raw_window_handle::RawWindowHandle) -> Option<super::WindowHandle> {
        match raw {
            raw_window_handle::RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
            _ => None,
        }
    }

    fn apply_locked_style(&self, hwnd: super::WindowHandle, locked: bool) {
        unsafe {
            let prev = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let mut new = (prev | (WS_EX_TOOLWINDOW.0 as isize)) & !(WS_EX_APPWINDOW.0 as isize);
            if locked {
                new |= WS_EX_TRANSPARENT.0 as isize;
            } else {
                new &= !(WS_EX_TRANSPARENT.0 as isize);
            }
            if new != prev {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOP,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
        }
    }

    fn apply_outer_position(&self, hwnd: super::WindowHandle, pos: (i32, i32)) {
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                HWND_TOP,
                pos.0,
                pos.1,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    fn refresh_stay_on_top(&self, hwnd: super::WindowHandle) {
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    fn window_outer_position(&self, hwnd: super::WindowHandle) -> Option<(i32, i32)> {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
        Some((rect.left, rect.top))
    }

    fn cursor_position(&self) -> Option<(i32, i32)> {
        let mut pt = POINT::default();
        unsafe { GetCursorPos(&mut pt) }.ok()?;
        Some((pt.x, pt.y))
    }

    fn has_transparent_style(&self, hwnd: super::WindowHandle) -> bool {
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            (ex & (WS_EX_TRANSPARENT.0 as isize)) != 0
        }
    }

    fn install_display_change_hook(&self, hwnd: super::WindowHandle, ctx: Arc<AppContext>) {
        use windows::Win32::UI::WindowsAndMessaging::GWL_WNDPROC;
        WND_CTX.with(|cell| {
            *cell.borrow_mut() = Some(ctx);
        });
        unsafe {
            let prev = SetWindowLongPtrW(
                hwnd,
                GWL_WNDPROC,
                monitor_wnd_proc as *const () as isize,
            );
            if prev != 0 {
                let prev_wndproc: WNDPROC = std::mem::transmute(prev);
                set_prev_wnd_proc(prev_wndproc);
            }
        }
    }
}

static WT_HOOK: OnceLock<Mutex<Option<isize>>> = OnceLock::new();
static WT_TARGET_TITLE: OnceLock<Mutex<String>> = OnceLock::new();

fn get_wt_hook_lock() -> &'static Mutex<Option<isize>> {
    WT_HOOK.get_or_init(|| Mutex::new(None))
}

fn get_wt_target_title() -> &'static Mutex<String> {
    WT_TARGET_TITLE.get_or_init(|| Mutex::new(String::from("MusicFoxTerminal")))
}

// 托盘菜单防抖：菜单关闭后 250ms 内忽略右键 Click 事件
// 避免 tray-icon 在菜单关闭瞬间把 MouseLeave/MouseHover 误翻译为 Click
static LAST_TRAY_MENU_SHOW_AT: OnceLock<Mutex<Instant>> = OnceLock::new();
const TRAY_MENU_DEBOUNCE: Duration = Duration::from_millis(250);

fn get_last_tray_menu_show() -> &'static Mutex<Instant> {
    LAST_TRAY_MENU_SHOW_AT.get_or_init(|| Mutex::new(Instant::now() - Duration::from_secs(60)))
}

fn mark_tray_menu_shown() {
    if let Ok(mut t) = get_last_tray_menu_show().lock() {
        *t = Instant::now();
    }
}

fn should_suppress_tray_menu() -> bool {
    if let Ok(t) = get_last_tray_menu_show().lock() {
        t.elapsed() < TRAY_MENU_DEBOUNCE
    } else {
        false
    }
}

unsafe extern "system" fn wt_minimizestart_proc(
    _h_win_event_hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    if event == EVENT_SYSTEM_MINIMIZESTART {
        if !hwnd.is_invalid() && IsWindow(hwnd).as_bool() {
            let target_title = {
                let guard = get_wt_target_title().lock().unwrap();
                guard.clone()
            };
            let mut buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, &mut buf);
            if len > 0 {
                let text = String::from_utf16_lossy(&buf[..len as usize]);
                if text.contains(&target_title) {
                    let hwnd_raw = hwnd.0 as isize;
                    thread::spawn(move || {
                        thread::sleep(Duration::from_millis(10));
                        let target_hwnd = HWND(hwnd_raw as *mut _);
                        unsafe {
                            let _ = ShowWindow(target_hwnd, SW_RESTORE);
                            let _ = SetLayeredWindowAttributes(target_hwnd, COLORREF(0), 0, LWA_ALPHA);
                        }
                    });
                }
            }
        }
    }
}

fn find_window_by_title_contains(target: &str) -> Option<HWND> {
    let title_w = to_wstring(target);
    let h = unsafe { FindWindowW(None, windows::core::PCWSTR(title_w.as_ptr())) };
    if let Ok(hwnd) = h {
        if !hwnd.is_invalid() && unsafe { IsWindow(hwnd).as_bool() } {
            return Some(hwnd);
        }
    }

    struct SearchContext {
        target: String,
        found: Option<HWND>,
    }
    let mut ctx = SearchContext {
        target: target.to_string(),
        found: None,
    };

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> windows::Win32::Foundation::BOOL {
        let ctx = &mut *(lparam.0 as *mut SearchContext);
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len > 0 {
            let text = String::from_utf16_lossy(&buf[..len as usize]);
            if text.contains(&ctx.target) {
                ctx.found = Some(hwnd);
                return windows::Win32::Foundation::BOOL(0);
            }
        }
        windows::Win32::Foundation::BOOL(1)
    }

    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut SearchContext as isize));
    }
    ctx.found
}

impl PlatformWt for WindowsBackend {
    fn find_wt_window(&self, title: &str) -> Option<super::WindowHandle> {
        find_window_by_title_contains(title)
    }

    fn launch_wt(&self, app_dir: &str, title: &str, musicfox_path: &str) -> Result<(), String> {
        let mut cmd = std::process::Command::new("wt.exe");
        cmd.arg("-w")
            .arg("new")
            .arg("-d")
            .arg(app_dir)
            .arg("--title")
            .arg(title)
            .arg(musicfox_path);
        cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
    }

    fn apply_wt_hosted_style(&self, hwnd: super::WindowHandle) {
        unsafe {
            let prev_ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let new_ex = (prev_ex | (WS_EX_TOOLWINDOW.0 as isize) | (WS_EX_LAYERED.0 as isize)) & !(WS_EX_APPWINDOW.0 as isize);
            if new_ex != prev_ex {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex);
            }

            let prev_style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            let new_style = prev_style
                & !(WS_CAPTION.0 as isize)
                & !(WS_SYSMENU.0 as isize)
                & !(WS_MAXIMIZEBOX.0 as isize)
                & !(WS_MINIMIZEBOX.0 as isize);
            if new_style != prev_style {
                SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
            }

            let sys_menu = GetSystemMenu(hwnd, false);
            if !sys_menu.is_invalid() {
                let _ = EnableMenuItem(sys_menu, SC_CLOSE, MF_GRAYED | MF_BYCOMMAND);
            }

            let _ = SetWindowPos(
                hwnd,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }

    fn set_wt_alpha(&self, hwnd: super::WindowHandle, alpha: u8) {
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            if (ex & (WS_EX_LAYERED.0 as isize)) == 0 {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | (WS_EX_LAYERED.0 as isize));
            }
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
        }
    }

    fn is_wt_hidden(&self, hwnd: super::WindowHandle) -> bool {
        unsafe {
            let mut cr_key = COLORREF(0);
            let mut alpha = 255u8;
            let mut flags = LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
            if GetLayeredWindowAttributes(hwnd, Some(&mut cr_key), Some(&mut alpha), Some(&mut flags)).is_ok() {
                alpha == 0
            } else {
                false
            }
        }
    }

    fn activate_wt(&self, hwnd: super::WindowHandle) {
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
        }
    }

    fn install_wt_minimize_hook(&self, title: String) {
        {
            let mut target = get_wt_target_title().lock().unwrap();
            *target = title;
        }
        let mut hook_guard = get_wt_hook_lock().lock().unwrap();
        if hook_guard.is_none() {
            let hook = unsafe {
                SetWinEventHook(
                    EVENT_SYSTEM_MINIMIZESTART,
                    EVENT_SYSTEM_MINIMIZESTART,
                    windows::Win32::Foundation::HMODULE(std::ptr::null_mut()),
                    Some(wt_minimizestart_proc),
                    0,
                    0,
                    0, // WINEVENT_OUTOFCONTEXT
                )
            };
            if !hook.is_invalid() {
                *hook_guard = Some(hook.0 as isize);
            }
        }
    }

    fn shutdown_wt(&self, title: &str) {
        {
            let mut hook_guard = get_wt_hook_lock().lock().unwrap();
            if let Some(hook_val) = hook_guard.take() {
                unsafe {
                    let _ = UnhookWinEvent(HWINEVENTHOOK(hook_val as *mut _));
                }
            }
        }
        if let Some(hwnd) = self.find_wt_window(title) {
            unsafe {
                let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
    }

    fn uninstall_wt_minimize_hook(&self, _title: &str) {
        // 仅卸载事件钩子，不发送 WM_CLOSE 关闭 WT 窗口
        let mut hook_guard = get_wt_hook_lock().lock().unwrap();
        if let Some(hook_val) = hook_guard.take() {
            unsafe {
                let _ = UnhookWinEvent(HWINEVENTHOOK(hook_val as *mut _));
            }
        }
    }
}

pub fn restart_lyric_app() {
    thread::spawn(|| {
        // 1. 查找已存在的歌词主窗口并发送 WM_CLOSE 促使其优雅退出
        let title_w = to_wstring("lyric-for-musicfox");
        if let Ok(hwnd) = unsafe { FindWindowW(None, windows::core::PCWSTR(title_w.as_ptr())) } {
            if !hwnd.is_invalid() {
                unsafe {
                    let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
        }

        // 2. 稍作等待，确保旧进程释放 Mutex 与 UDP socket
        thread::sleep(Duration::from_millis(250));

        // 3. 启动全新的歌词主进程
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
    });
}
