//! 拖动状态机：仅 locked=false 时启用；按下即开始；松开写 state.pos_x/y。

use crate::context::Signals;
use crate::lyric::state::LyricState;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragPhase {
    #[default]
    Idle,
    Active {
        anchor_physical: (i32, i32),
        anchor_window: (i32, i32),
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DragState {
    pub phase: DragPhase,
}

impl DragState {
    pub fn new() -> Self {
        Self {
            phase: DragPhase::Idle,
        }
    }
}

/// 纯函数：从按下时的光标/窗口锚点 + 当前光标位置计算新窗口位置。
pub fn compute_new_position(
    anchor_physical: (i32, i32),
    anchor_window: (i32, i32),
    current_physical: (i32, i32),
) -> (i32, i32) {
    let dx = current_physical.0 - anchor_physical.0;
    let dy = current_physical.1 - anchor_physical.1;
    (anchor_window.0 + dx, anchor_window.1 + dy)
}

/// 消费一帧指针状态。返回需要 `SetWindowPos` 的新外框位置。
///
/// `left_pressed` 仅在按下那一帧为 true；按住期间每帧只要有光标位置就会更新。
/// 松开时把终态位置写入 `state.pos_x/y`（内存，不写盘）。
pub fn process_pointer_input(
    drag: &mut DragState,
    state: &mut LyricState,
    signals: &Signals,
    config_locked: bool,
    current_physical: Option<(i32, i32)>,
    left_pressed: bool,
    left_released: bool,
    current_outer_position: (i32, i32),
) -> Option<(i32, i32)> {
    if config_locked {
        drag.phase = DragPhase::Idle;
        signals.is_dragging.store(false, Ordering::SeqCst);
        return None;
    }

    if left_pressed && drag.phase == DragPhase::Idle {
        if let Some(cur) = current_physical {
            drag.phase = DragPhase::Active {
                anchor_physical: cur,
                anchor_window: current_outer_position,
            };
            signals.is_dragging.store(true, Ordering::SeqCst);
        }
    }

    let DragPhase::Active {
        anchor_physical,
        anchor_window,
    } = drag.phase
    else {
        signals.is_dragging.store(false, Ordering::SeqCst);
        return None;
    };

    let computed =
        current_physical.map(|cur| compute_new_position(anchor_physical, anchor_window, cur));

    if left_released {
        let final_pos = computed.unwrap_or(current_outer_position);
        state.pos_x = final_pos.0;
        state.pos_y = final_pos.1;
        state.target_pos_x = final_pos.0;
        state.target_pos_y = final_pos.1;
        drag.phase = DragPhase::Idle;
        signals.is_dragging.store(false, Ordering::SeqCst);
        return Some(final_pos);
    }

    computed
}

/// 进程退出时若仍按住，把当前外框位置同步进内存状态。
pub fn flush_on_exit(
    drag: &mut DragState,
    state: &mut LyricState,
    signals: &Signals,
    current_outer_position: (i32, i32),
) {
    if matches!(drag.phase, DragPhase::Active { .. }) {
        state.pos_x = current_outer_position.0;
        state.pos_y = current_outer_position.1;
        state.target_pos_x = current_outer_position.0;
        state.target_pos_y = current_outer_position.1;
        drag.phase = DragPhase::Idle;
        signals.is_dragging.store(false, Ordering::SeqCst);
    }
}
