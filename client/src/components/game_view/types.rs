//! Internal game-view types shared by the component and its input handlers.

use super::geometry::client_to_canvas;
use crate::camera::CameraManager;
use crate::canvas::DraggedPiece;
use crate::math::Vec2;
use crate::reducer::GameStateReducer;
use common::types::PieceId;
use std::cell::RefCell;
use std::rc::Rc;
use web_sys::HtmlCanvasElement;
use yew::prelude::*;

/// Pointer travel, in pixels, after which a press counts as a drag rather than a tap.
pub(super) const TAP_SLOP_PX: f64 = 10.0;

/// Pointer-down state for distinguishing taps from pans.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DragStart {
    pub pos: Vec2,
    pub allow_panning: bool,
}

/// One of the local player's pieces picked up by a press, following the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HeldPiece {
    pub piece_id: PieceId,
    /// Pointer position of the press.
    start: Vec2,
    /// Latest pointer position.
    pub pos: Vec2,
    /// True once the pointer travelled far enough for the press to be a drag.
    pub dragged: bool,
    /// Selection before the press, which a tap is resolved against.
    pub previous_selected: Option<PieceId>,
}

impl HeldPiece {
    /// Picks up `piece_id` at pointer position `pos`.
    pub(super) fn new(piece_id: PieceId, pos: Vec2, previous_selected: Option<PieceId>) -> Self {
        Self {
            piece_id,
            start: pos,
            pos,
            dragged: false,
            previous_selected,
        }
    }

    /// Moves the piece with the pointer; once dragged, it stays dragged.
    pub(super) fn follow(&mut self, pos: Vec2) {
        self.pos = pos;
        self.dragged |= (pos - self.start).length() > TAP_SLOP_PX;
    }
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
    pub held: Rc<RefCell<Option<HeldPiece>>>,
}

impl ViewHandles {
    /// Ends any drag in progress and stops camera inertia.
    pub(super) fn cancel_drag(&self) {
        self.cancel_hold();
        self.drag_start.set(None);
        self.camera.borrow_mut().velocity = Vec2::ZERO;
        *self.did_pan.borrow_mut() = false;
    }

    /// Drops the held piece without moving it; it stays selected.
    pub(super) fn cancel_hold(&self) {
        self.held.borrow_mut().take();
    }

    /// Returns true while a held piece is being dragged.
    pub(super) fn is_dragging_piece(&self) -> bool {
        self.held.borrow().is_some_and(|held| held.dragged)
    }

    /// Returns the held piece to draw under the pointer once it is being dragged.
    pub(super) fn dragged_piece(&self) -> Option<DraggedPiece> {
        let held = (*self.held.borrow()).filter(|held| held.dragged)?;
        let canvas = self.canvas_ref.cast::<HtmlCanvasElement>()?;
        Some(DraggedPiece {
            piece_id: held.piece_id,
            canvas_pos: client_to_canvas(held.pos, &canvas),
        })
    }

    /// Marks the drag state seen by timer-driven callbacks as not dragging.
    pub(super) fn mark_not_dragging(&self) {
        if let Ok(mut latest) = self.latest.try_borrow_mut() {
            latest.is_dragging = false;
        }
    }
}
