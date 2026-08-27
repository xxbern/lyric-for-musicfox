//! 歌词状态聚合（P3：UDP 实现）

pub mod state;
pub mod udp;

pub use state::{LineInfo, LyricState, LyricWord};
pub use udp::{UdpLineInfo, UdpLyricPayload, UdpLyricWord, UdpSongInfo};
