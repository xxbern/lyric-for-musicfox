//! UDP communication module for receiving lyrics from go-musicfox.
//! Matches Phase P3 requirements.

use std::net::UdpSocket;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use crate::lyric::state::LyricState;

fn default_type() -> String {
    "lyric_update".to_string()
}

fn default_playing() -> bool {
    false
}

fn default_time_ms() -> i64 {
    0
}

fn default_volume() -> i32 {
    0
}

fn default_mode() -> String {
    String::new()
}

fn default_song_id() -> i64 {
    0
}

fn default_song_name() -> String {
    String::new()
}

fn default_song_artist() -> String {
    String::new()
}

fn default_album() -> String {
    String::new()
}

fn default_pic_url() -> String {
    String::new()
}

fn default_duration() -> i64 {
    0
}

fn default_is_favorite() -> bool {
    false
}

fn default_line_text() -> String {
    String::new()
}

fn default_words() -> Vec<UdpLyricWord> {
    Vec::new()
}

fn default_word() -> String {
    String::new()
}

fn default_word_start_time() -> i64 {
    0
}

fn default_word_duration() -> i64 {
    0
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UdpLyricPayload {
    #[serde(rename = "type", default = "default_type")]
    pub type_: String,
    #[serde(default = "default_playing")]
    pub playing: bool,
    #[serde(default = "default_time_ms")]
    pub time_ms: i64,
    #[serde(default = "default_volume")]
    pub volume: i32,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub song: UdpSongInfo,
    #[serde(default)]
    pub current_line: UdpLineInfo,
    #[serde(default)]
    pub next_line: UdpLineInfo,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct UdpSongInfo {
    #[serde(default = "default_song_id")]
    pub id: i64,
    #[serde(default = "default_song_name")]
    pub name: String,
    #[serde(default = "default_song_artist")]
    pub artist: String,
    #[serde(default = "default_album")]
    pub album: String,
    #[serde(default = "default_pic_url")]
    pub pic_url: String,
    #[serde(default = "default_duration")]
    pub duration: i64,
    #[serde(default = "default_is_favorite")]
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct UdpLineInfo {
    #[serde(default = "default_line_text")]
    pub text: String,
    #[serde(default = "default_words")]
    pub words: Vec<UdpLyricWord>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct UdpLyricWord {
    #[serde(default = "default_word")]
    pub word: String,
    #[serde(default = "default_word_start_time")]
    pub start_time: i64,
    #[serde(default = "default_word_duration")]
    pub duration: i64,
}

#[derive(Debug)]
pub enum RecvError {
    TooLarge { len: usize },
    Parse(serde_json::Error),
}

impl std::fmt::Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { len } => write!(f, "Packet size {} exceeds 8KB limit", len),
            Self::Parse(err) => write!(f, "JSON parse error: {}", err),
        }
    }
}

impl std::error::Error for RecvError {}

/// Parses a UDP packet to UdpLyricPayload.
/// bytes.len() > 8192 is unreachable in the recv_loop because buf is exactly 8192 bytes,
/// but it is tested in unit tests to verify proper handling of over-sized packet limits.
pub fn parse_packet(bytes: &[u8]) -> Result<UdpLyricPayload, RecvError> {
    if bytes.len() > 8192 {
        return Err(RecvError::TooLarge { len: bytes.len() });
    }
    match serde_json::from_slice(bytes) {
        Ok(payload) => Ok(payload),
        Err(e) => {
            if let Ok(text) = std::str::from_utf8(bytes) {
                let trimmed = text.trim_end_matches(['\r', '\n']).trim();
                if !trimmed.is_empty() && !trimmed.starts_with('{') {
                    return Ok(UdpLyricPayload {
                        type_: "lyric_update".to_string(),
                        playing: true,
                        time_ms: 0,
                        volume: 100,
                        mode: String::new(),
                        song: UdpSongInfo::default(),
                        current_line: UdpLineInfo {
                            text: trimmed.to_string(),
                            words: Vec::new(),
                        },
                        next_line: UdpLineInfo::default(),
                    });
                }
            }
            Err(RecvError::Parse(e))
        }
    }
}

/// Checks if the payload's type is "lyric_update".
pub fn is_lyric_update(payload: &UdpLyricPayload) -> bool {
    payload.type_ == "lyric_update"
}

/// Update the shared state with received payload info.
pub fn apply_to_state(state: &mut LyricState, payload: &UdpLyricPayload) {
    state.playing = payload.playing;
    state.time_ms = payload.time_ms;
    state.song_name = payload.song.name.clone();
    state.song_artist = payload.song.artist.clone();
    state.current_line.text = Arc::from(payload.current_line.text.as_str());
    state.next_line.text = Arc::from(payload.next_line.text.as_str());
    state.current_line.is_placeholder = payload.current_line.text.is_empty();
}

/// Bind the UDP port receive_port on 0.0.0.0.
pub fn bind(port: u16) -> std::io::Result<UdpSocket> {
    let addr = format!("0.0.0.0:{}", port);
    UdpSocket::bind(&addr)
}

/// Main loop for receiving and updating loop using blocking UdpSocket.
pub fn recv_loop(socket: UdpSocket, ctx: std::sync::Arc<crate::context::AppContext>) {
    let mut buf = [0u8; 8200];
    loop {
        let result = catch_unwind(AssertUnwindSafe(|| match socket.recv_from(&mut buf) {
            Ok((len, _)) => {
                let bytes = &buf[..len];
                match parse_packet(bytes) {
                    Ok(payload) => {
                        if !is_lyric_update(&payload) {
                            log::warn!(
                                "drop UDP packet: type != lyric_update (got: {})",
                                payload.type_
                            );
                            return;
                        }
                        match ctx.state.write() {
                            Ok(mut state_guard) => {
                                let prev_text = state_guard.current_line.text.clone();
                                let prev_playing = state_guard.playing;
                                apply_to_state(&mut state_guard, &payload);
                                if prev_text != state_guard.current_line.text
                                    || prev_playing != state_guard.playing
                                {
                                    ctx.event_bus
                                        .emit(crate::event_bus::AppEvent::LyricStateChanged);
                                    ctx.event_bus
                                        .emit(crate::event_bus::AppEvent::RequestRepaint);
                                }
                            }
                            Err(poisoned) => {
                                log::error!("LyricState RwLock is poisoned. Resolving poison state and applying.");
                                let mut state_guard = poisoned.into_inner();
                                let prev_text = state_guard.current_line.text.clone();
                                let prev_playing = state_guard.playing;
                                apply_to_state(&mut state_guard, &payload);
                                if prev_text != state_guard.current_line.text
                                    || prev_playing != state_guard.playing
                                {
                                    ctx.event_bus
                                        .emit(crate::event_bus::AppEvent::LyricStateChanged);
                                    ctx.event_bus
                                        .emit(crate::event_bus::AppEvent::RequestRepaint);
                                }
                            }
                        }
                    }
                    Err(RecvError::TooLarge { len }) => {
                        log::warn!("drop UDP packet: size {} > 8KB", len);
                    }
                    Err(RecvError::Parse(e)) => {
                        let text = String::from_utf8_lossy(bytes);
                        log::warn!(
                            "drop UDP packet: JSON parse error: {}, content: {:?}",
                            e,
                            text
                        );
                    }
                }
            }
            Err(e) => {
                log::error!("UDP socket recv_from error: {}", e);
            }
        }));
        if let Err(e) = result {
            log::error!("Panic captured in UDP recv loop: {:?}", e);
        }
    }
}
