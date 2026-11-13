//! Internal game-view types shared by the component and its input handlers.

use crate::camera::CameraManager;
use crate::math::Vec2;
use crate::reducer::GameStateReducer;
use common::types::PieceId;
use std::cell::RefCell;
use std::rc::Rc;
use yew::prelude::*;

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

/// Latest render inputs for timer-driven callbacks that outlive a single render.
pub(super) struct LatestStateSnapshot {
    pub reducer: UseReducerHandle<GameStateReducer>,
    pub is_dragging: bool,
}

/// Tracks frame timing to compute FPS values.
pub(super) struct FpsCounter {
    frames: u32,
    last_ms: f64,
}

impl FpsCounter {
    /// Starts counting at `now`.
    pub(super) fn new(now: f64) -> Self {
        Self {
            frames: 0,
            last_ms: now,
        }
    }

    /// Counts one frame at `now`, returning the FPS once per second.
    pub(super) fn frame(&mut self, now: f64) -> Option<u32> {
        self.frames += 1;
        let elapsed = now - self.last_ms;
        if elapsed < 1000.0 {
            return None;
        }
        let fps = (self.frames as f64 * 1000.0 / elapsed).round() as u32;
        self.frames = 0;
        self.last_ms = now;
        Some(fps)
    }
}

/// Handles shared between the game view and its input handlers.
#[derive(Clone)]
pub(super) struct ViewHandles {
    pub canvas_ref: NodeRef,
    pub camera: Rc<RefCell<CameraManager>>,
    pub cam_state: UseStateHandle<Vec2>,
    pub zoom_state: UseStateHandle<f64>,
    pub drag_start: UseStateHandle<Option<DragStart>>,
    pub did_pan: Rc<RefCell<bool>>,
    pub selected: UseStateHandle<Option<PieceId>>,
    pub latest: Rc<RefCell<LatestStateSnapshot>>,
    pub touch_gesture_active: Rc<RefCell<bool>>,
    pub last_tap: Rc<RefCell<Option<LastTap>>>,
}

impl ViewHandles {
    /// Ends any drag in progress and stops camera inertia.
    pub(super) fn cancel_drag(&self) {
        self.drag_start.set(None);
        self.camera.borrow_mut().velocity = Vec2::ZERO;
        *self.did_pan.borrow_mut() = false;
    }

    /// Marks the drag state seen by timer-driven callbacks as not dragging.
    pub(super) fn mark_not_dragging(&self) {
        if let Ok(mut latest) = self.latest.try_borrow_mut() {
            latest.is_dragging = false;
        }
    }
}
