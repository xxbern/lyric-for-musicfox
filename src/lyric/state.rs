//! 内存歌词状态。P1 由 LyricApp 直接持有裸值（P3 才升级 Arc<RwLock<_>>）。

use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct LyricState {
    pub current_line: LineInfo,
    pub next_line: LineInfo,
    pub song_name: String,
    pub song_artist: String,
    pub playing: bool,
    pub time_ms: i64,
    pub pos_x: i32,
    pub pos_y: i32,
    pub target_pos_x: i32,
    pub target_pos_y: i32,
}

#[derive(Debug, Clone, Default)]
pub struct LineInfo {
    pub text: Arc<str>,
    /// 仅用于区分启动占位符 alpha=0.3；不是 Config 字段。
    pub is_placeholder: bool,
    pub words: Vec<LyricWord>,
}

#[derive(Debug, Clone)]
pub struct LyricWord {
    pub word: String,
    pub start_time: i64,
    pub duration: i64,
}

impl LyricState {
    pub fn placeholder() -> Self {
        Self {
            current_line: LineInfo {
                text: Arc::from("......"),
                is_placeholder: true,
                words: Vec::new(),
            },
            ..Self::default()
        }
    }
}

/// 文本透明度：占位符优先于 playing。
pub fn text_alpha(is_placeholder: bool, playing: bool) -> f32 {
    if is_placeholder {
        0.3
    } else if playing {
        1.0
    } else {
        0.5
    }
}

use std::sync::atomic::AtomicBool;

#[derive(Clone, Debug, Default)]
pub struct SharedSignals {
    pub needs_reload: Arc<AtomicBool>,
    pub is_dragging: Arc<AtomicBool>,
    pub display_changed: Arc<AtomicBool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_sets_six_dots_and_flag() {
        let state = LyricState::placeholder();
        assert_eq!(&*state.current_line.text, "......");
        assert!(state.current_line.is_placeholder);
        assert!(!state.playing);
    }

    #[test]
    fn alpha_placeholder_beats_playing() {
        assert_eq!(text_alpha(true, true), 0.3);
        assert_eq!(text_alpha(true, false), 0.3);
        assert_eq!(text_alpha(false, true), 1.0);
        assert_eq!(text_alpha(false, false), 0.5);
    }
}
