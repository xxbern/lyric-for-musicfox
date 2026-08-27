//! Config codec: 严格类型 + 字段级 serde(default) + 配对 deserialize_with/serialize_with
//! 详见 dev.md §8.1 + req.md §8/§12.1
//!
//! Option 字段空串 ↔ None 约定：
//!   pos_x / pos_y  : 缺失或 `""` → None；序列化 None → `""`
//!   font_outline_color : 缺失 → Some("#000000")；`""` → None；序列化 None → `""`

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod load;
pub mod save;

// ===== 字段级 default 函数 =====
fn default_width() -> u32 {
    800
}
fn default_height() -> u32 {
    80
}
fn default_pos() -> Option<i32> {
    None
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_font_family() -> String {
    "Microsoft YaHei".into()
}
fn default_font_size() -> f32 {
    24.0
}
fn default_font_color() -> String {
    "#ffffff".into()
}
fn default_outline_color() -> Option<String> {
    Some("#000000".into())
}
fn default_outline_width() -> u32 {
    1
}
fn default_receive_port() -> u16 {
    16501
}
fn default_send_port() -> u16 {
    16502
}
fn default_window() -> WindowConfig {
    WindowConfig::default()
}
fn default_style() -> LyricStyleConfig {
    LyricStyleConfig::default()
}
fn default_system() -> SystemConfig {
    SystemConfig::default()
}
fn default_wt() -> WtConfig {
    WtConfig::default()
}
fn default_wt_musicfox_path() -> String {
    "C:\\Users\\xx\\app\\musicfox\\musicfox.exe".into()
}
fn default_wt_app_dir() -> String {
    "C:\\Users\\xx\\app".into()
}
pub(crate) fn default_wt_title() -> String {
    "MusicFoxTerminal".into()
}

// ===== pos_x / pos_y 专用 adapter：整数或空串 =====
#[derive(Deserialize)]
#[serde(untagged)]
enum IntOrEmpty {
    Int(i32),
    Empty(String),
}

pub fn de_pos<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i32>, D::Error> {
    match IntOrEmpty::deserialize(d)? {
        IntOrEmpty::Int(v) => Ok(Some(v)),
        IntOrEmpty::Empty(s) if s.is_empty() => Ok(None),
        IntOrEmpty::Empty(_) => Err(serde::de::Error::custom(
            "position must be integer or empty string",
        )),
    }
}

pub fn ser_pos<S: Serializer>(v: &Option<i32>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(v) => s.serialize_i32(*v),
        None => s.serialize_str(""),
    }
}

// ===== font_outline_color 专用 adapter：空串 → None，其他 → Some(s) =====
pub fn de_outline<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let s = String::deserialize(d)?;
    Ok(if s.is_empty() { None } else { Some(s) })
}

pub fn ser_outline<S: Serializer>(v: &Option<String>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(v.as_deref().unwrap_or(""))
}

// ===== Config struct =====
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_window")]
    pub window: WindowConfig,
    #[serde(default = "default_style")]
    pub lyric_style: LyricStyleConfig,
    #[serde(default = "default_system")]
    pub system: SystemConfig,
    #[serde(default = "default_wt")]
    pub wt: WtConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowConfig {
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(
        default = "default_pos",
        deserialize_with = "de_pos",
        serialize_with = "ser_pos"
    )]
    pub pos_x: Option<i32>,
    #[serde(
        default = "default_pos",
        deserialize_with = "de_pos",
        serialize_with = "ser_pos"
    )]
    pub pos_y: Option<i32>,
    #[serde(default = "default_true")]
    pub stay_on_top: bool,
    #[serde(default = "default_true", skip_serializing)]
    pub frame_less: bool,
    #[serde(default = "default_false")]
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricStyleConfig {
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default)]
    pub font_bold: bool,
    #[serde(default)]
    pub font_italic: bool,
    #[serde(default = "default_font_color")]
    pub font_color: String,
    #[serde(
        default = "default_outline_color",
        deserialize_with = "de_outline",
        serialize_with = "ser_outline"
    )]
    pub font_outline_color: Option<String>,
    #[serde(default = "default_outline_width")]
    pub font_outline_width: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemConfig {
    #[serde(default = "default_receive_port")]
    pub receive_port: u16,
    #[serde(default = "default_send_port")]
    pub send_port: u16,
    #[serde(default = "default_false")]
    pub log_enabled: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: default_width(),
            height: default_height(),
            pos_x: default_pos(),
            pos_y: default_pos(),
            stay_on_top: default_true(),
            frame_less: default_true(),
            locked: default_false(),
        }
    }
}

impl Default for LyricStyleConfig {
    fn default() -> Self {
        Self {
            font_family: default_font_family(),
            font_size: default_font_size(),
            font_bold: false,
            font_italic: false,
            font_color: default_font_color(),
            font_outline_color: default_outline_color(),
            font_outline_width: default_outline_width(),
        }
    }
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            receive_port: default_receive_port(),
            send_port: default_send_port(),
            log_enabled: default_false(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            lyric_style: LyricStyleConfig::default(),
            system: SystemConfig::default(),
            wt: WtConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WtConfig {
    #[serde(default = "default_wt_musicfox_path")]
    pub musicfox_path: String,
    #[serde(default = "default_wt_app_dir")]
    pub app_dir: String,
    #[serde(default = "default_wt_title")]
    pub title: String,
}

impl Default for WtConfig {
    fn default() -> Self {
        Self {
            musicfox_path: default_wt_musicfox_path(),
            app_dir: default_wt_app_dir(),
            title: default_wt_title(),
        }
    }
}
