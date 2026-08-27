//! 字体生命周期服务（P4-R：零常驻定向查询，替代 fontdb 全库扫描）。
//!
//! - 存在性检查：GDI `EnumFontFamiliesExW`（不加载字体文件）
//! - 字节提取：注册表 Fonts 键定位文件路径后仅读取目标单文件
//! - 非 Windows 平台：未知/不可用（resolve 保持原样、字节返回 None）

use std::sync::Arc;
use crate::context::AppContext;

pub const FALLBACK_FONT_FAMILY: &str = "Microsoft YaHei";

#[cfg(windows)]
mod imp {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, LPARAM};
    use windows::Win32::Graphics::Gdi::{
        EnumFontFamiliesExW, GetDC, ReleaseDC, DEFAULT_CHARSET, FONTENUMPROCW,
        HDC, LOGFONTW, TEXTMETRICW,
    };
    use windows::Win32::System::Registry::{
        RegCloseKey, RegEnumValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
        KEY_READ,
    };

    const LF_FACESIZE: usize = 32;

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().take(LF_FACESIZE - 1).collect()
    }

    unsafe extern "system" fn mark_found_cb(
        _lp: *const LOGFONTW,
        _ntm: *const TEXTMETRICW,
        _font_type: u32,
        _lparam: LPARAM,
    ) -> i32 {
        FOUND.store(true, Ordering::Relaxed);
        0 // 停止枚举
    }

    static FOUND: AtomicBool = AtomicBool::new(false);

    /// GDI 枚举：按 face name 过滤，不加载字体文件，零常驻。
    pub(super) fn family_installed(name: &str) -> bool {
        unsafe {
            let hdc: HDC = GetDC(None);
            let mut lf = LOGFONTW::default();
            lf.lfCharSet = DEFAULT_CHARSET;
            for (i, c) in to_wide(name).iter().enumerate() {
                lf.lfFaceName[i] = *c;
            }
            FOUND.store(false, Ordering::Relaxed);
            let cb: FONTENUMPROCW = Some(mark_found_cb);
            EnumFontFamiliesExW(hdc, &lf, cb, LPARAM(0), 0);
            ReleaseDC(None, hdc);
            FOUND.load(Ordering::Relaxed)
        }
    }

    /// 枚举全部 family 名称（设置窗口下拉框用）。
    pub(super) fn list_families() -> Vec<String> {
        unsafe extern "system" fn collect_cb(
            lp: *const LOGFONTW,
            _ntm: *const TEXTMETRICW,
            _font_type: u32,
            lparam: LPARAM,
        ) -> i32 {
            unsafe {
                let e = &*lp;
                let names = &mut *(lparam.0 as *mut Vec<String>);
                let end = e
                    .lfFaceName
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(LF_FACESIZE);
                let s = String::from_utf16_lossy(&e.lfFaceName[..end]);
                if !s.is_empty() {
                    names.push(s);
                }
            }
            1 // 继续枚举
        }

        let mut names: Vec<String> = Vec::new();
        unsafe {
            let hdc: HDC = GetDC(None);
            let lf = LOGFONTW {
                lfCharSet: DEFAULT_CHARSET,
                ..Default::default()
            };
            let cb: FONTENUMPROCW = Some(collect_cb);
            EnumFontFamiliesExW(
                hdc,
                &lf,
                cb,
                LPARAM(&mut names as *mut Vec<String> as isize),
                0,
            );
            ReleaseDC(None, hdc);
        }
        names.sort();
        names.dedup();
        names
    }

    fn fonts_key(hkey_root: HKEY) -> Option<HKEY> {
        let sub: Vec<u16> =
            format!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts\0")
                .encode_utf16()
                .collect();
        let mut hkey = HKEY::default();
        let res = unsafe {
            RegOpenKeyExW(hkey_root, PCWSTR(sub.as_ptr()), 0, KEY_READ, &mut hkey)
        };
        if res == ERROR_SUCCESS {
            Some(hkey)
        } else {
            crate::diag::record(&format!(
                "reg_open_fail_{}_code_{}",
                if hkey_root == HKEY_LOCAL_MACHINE { "hklm" } else { "hkcu" },
                res.0
            ));
            None
        }
    }

    fn enum_fonts_key(hkey: HKEY, fam_l: &str, best: &mut Option<(bool, PathBuf)>) {
        let mut idx = 0u32;
        loop {
            let mut name_buf = [0u16; 512];
            let mut data_buf = [0u16; 512];
            let mut name_len = name_buf.len() as u32;
            let mut data_len = data_buf.len() as u32;
            let mut vtype: u32 = 0;
            let res = unsafe {
                RegEnumValueW(
                    hkey,
                    idx,
                    windows::core::PWSTR(name_buf.as_mut_ptr()),
                    &mut name_len,
                    None,
                    Some(&mut vtype),
                    Some(data_buf.as_mut_ptr().cast()),
                    Some(&mut data_len),
                )
            };
            if res == ERROR_NO_MORE_ITEMS {
                break;
            }
            if res != ERROR_SUCCESS {
                idx += 1;
                continue;
            }
            idx += 1;

            let value_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
            let file = String::from_utf16_lossy(&data_buf[..data_len as usize])
                .trim_end_matches('\0')
                .to_string();
            let fl = file.to_ascii_lowercase();
            if !(fl.ends_with(".ttf") || fl.ends_with(".ttc") || fl.ends_with(".otf")) {
                continue;
            }

            // "Microsoft YaHei & Microsoft YaHei UI (TrueType)" → base
            let base = match value_name.rfind('(') {
                Some(p) => value_name[..p].trim_end(),
                None => value_name.trim(),
            };
            let comps: Vec<String> = base.split(" & ").map(|c| c.trim().to_lowercase()).collect();
            let exact = comps.iter().any(|c| *c == fam_l);
            let loose = exact || comps.iter().any(|c| c.starts_with(&fam_l));

            if loose {
                let better = exact && !best.as_ref().map(|(e, _)| *e).unwrap_or(false);
                if best.is_none() || better {
                    let p = PathBuf::from(&file);
                    let path = if p.is_absolute() { p } else {
                        PathBuf::from("C:\\Windows\\Fonts").join(p)
                    };
                    crate::diag::record(&format!(
                        "reg_candidate_{:?}_exact_{}",
                        file, exact
                    ));
                    *best = Some((exact, path));
                }
            }
        }
    }

    /// 注册表 Fonts 键：family 名 → 字体文件绝对路径。
    /// 先查 HKCU（按用户安装的字体，数据为绝对路径），再查 HKLM（全机字体）。
    pub(super) fn locate_font_file(family: &str) -> Option<PathBuf> {
        let fam_l = family.to_lowercase();
        let mut best: Option<(bool, PathBuf)> = None; // (精确匹配?, 路径)

        if let Some(hkey) = fonts_key(HKEY_CURRENT_USER) {
            enum_fonts_key(hkey, &fam_l, &mut best);
            unsafe {
                let _ = RegCloseKey(hkey);
            }
        }
        if best.as_ref().map(|(e, _)| *e).unwrap_or(false) {
            return best.map(|(_, p)| p);
        }

        if let Some(hkey) = fonts_key(HKEY_LOCAL_MACHINE) {
            enum_fonts_key(hkey, &fam_l, &mut best);
            unsafe {
                let _ = RegCloseKey(hkey);
            }
        }

        match best {
            Some((_, p)) => Some(p),
            None => {
                crate::diag::record("reg_no_match");
                None
            }
        }
    }

    pub(super) fn read_font_file(path: &PathBuf) -> Option<Vec<u8>> {
        match std::fs::read(path) {
            Ok(b) => Some(b),
            Err(e) => {
                crate::diag::record(&format!("font_read_fail_{}_{}", e.kind(), path.display()));
                None
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    /// 非 Windows 平台无法确认 → 返回 None（unknown），调用方按"未验证"处理。
    #[allow(dead_code)]
    pub(super) fn family_installed(_name: &str) -> Option<bool> {
        None
    }

    pub(super) fn list_families() -> Vec<String> {
        Vec::new()
    }

    pub(super) fn load_font_bytes(_family: &str, _bold: bool, _italic: bool) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(windows)]
fn installed(name: &str) -> Option<bool> {
    use std::sync::OnceLock;
    static FIRST: OnceLock<()> = OnceLock::new();
    let r = imp::family_installed(name);
    let _ = FIRST.get_or_init(|| {
        crate::diag::record(&format!("gdi_first_check_{}_{}", name, r));
    });
    Some(r)
}

#[cfg(not(windows))]
fn installed(_name: &str) -> Option<bool> {
    None
}

pub fn font_family_installed(name: &str) -> bool {
    installed(name).unwrap_or(false)
}

/// 平台**确定性判定缺失**时为 true；无法验证（非 Windows）不视为缺失。
pub fn font_family_certainly_missing(name: &str) -> bool {
    matches!(installed(name), Some(false))
}

/// 配置字体不存在时回退 Microsoft YaHei；不修改传入的配置值。
pub fn resolve_runtime_font(requested: &str) -> String {
    match installed(requested) {
        Some(true) => requested.to_string(),
        Some(false) => {
            log::warn!(
                "font family `{requested}` not found; falling back to {FALLBACK_FONT_FAMILY}"
            );
            FALLBACK_FONT_FAMILY.to_string()
        }
        // 无法验证（非 Windows）：保持请求值，交由渲染端回退
        None => requested.to_string(),
    }
}

/// 仅读取目标 family 的单个字体文件（P4-R：无全库常驻）。
/// 注：返回完整文件字节（ttc 含多 face 时取 index 0）。
pub fn load_font_bytes(family: &str, bold: bool, italic: bool) -> Option<Vec<u8>> {
    #[cfg(windows)]
    {
        let _ = (bold, italic); // 风格合成由消费端处理；保持旧行为只取主文件
        let path = imp::locate_font_file(family)?;
        imp::read_font_file(&path)
    }
    #[cfg(not(windows))]
    {
        let _ = (family, bold, italic);
        imp::load_font_bytes(family, bold, italic)
    }
}

pub(crate) fn system_families() -> Vec<String> {
    imp::list_families()
}

/// 字体服务：仅在内部持有 `Arc<AppContext>`。
#[derive(Clone)]
pub struct FontService {
    #[allow(dead_code)]
    ctx: Arc<AppContext>,
}

impl FontService {
    pub fn new(ctx: Arc<AppContext>) -> Self {
        Self { ctx }
    }

    /// 解析请求的字体族；不存在时回退默认字体。
    pub fn resolve_family(&self, requested: &str) -> String {
        resolve_runtime_font(requested)
    }

    pub fn get_font_bytes(&self, family: &str, bold: bool, italic: bool) -> Option<Vec<u8>> {
        load_font_bytes(family, bold, italic)
    }
}
