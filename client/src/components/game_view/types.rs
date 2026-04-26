//! Internal game-view types used by the main component implementation.

use crate::math::Vec2;
use crate::reducer::GameStateReducer;
use common::types::BoardCoord;
use yew::prelude::UseReducerHandle;

/// Pointer-down state for distinguishing taps from pans.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DragStart {
    pub pos: Vec2,
    pub allow_panning: bool,
}

/// Tracks the last tap to detect double-tap gestures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LastTap {
    pub time_ms: f64,
    pub pos: Vec2,
}

/// Pointer-down input payload.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct InputStart {
    pub pos: Vec2,
    pub is_right_click: bool,
}

/// Pointer-move input payload.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct InputMove {
    pub pos: Vec2,
}

/// Pointer-up input payload.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct InputEnd {
    pub pos: Vec2,
    pub is_right_click: bool,
}

/// Snapshot of reducer state used by background callbacks.
#[derive(Clone)]
pub(super) struct LatestStateSnapshot {
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub is_dragging: bool,
}

impl LatestStateSnapshot {
    /// Creates a snapshot for timer-driven renders.
    pub fn new(reducer: UseReducerHandle<GameStateReducer>, is_dragging: bool) -> Self {
        Self {
            reducer,
            is_dragging,
        }
    }

    /// Updates the cached reducer handle and dragging flag.
    pub fn update(&mut self, reducer: UseReducerHandle<GameStateReducer>, is_dragging: bool) {
        self.reducer = reducer;
        self.is_dragging = is_dragging;
    }
}

/// Tracks frame timing to compute FPS values.
pub(super) struct FpsCounter {
    pub frames: u32,
    pub last_ms: f64,
}

impl FpsCounter {
    /// Initializes the FPS counter with the current timestamp.
    pub fn new() -> Self {
        let now = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        Self {
            frames: 0,
            last_ms: now,
        }
    }
}

/// Animation state for a piece transitioning between tiles.
#[derive(Clone)]
pub(super) struct PieceAnim {
    pub start: BoardCoord,
    pub end: BoardCoord,
    pub started_at: f64,
}
