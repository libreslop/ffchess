//! Touch input: single-finger taps, pans, and piece drags, two-finger pinch zoom, and
//! double-tap fullscreen.

use super::geometry::is_ui_exempt_target;
use super::input::PointerHandlers;
use super::types::{InputEnd, InputMove, InputStart, LastTap, TAP_SLOP_PX, ViewHandles};
use crate::math::{Vec2, vec2};
use crate::reducer::GameStateReducer;
use crate::utils::{performance_now, request_fullscreen};
use web_sys::{HtmlCanvasElement, Touch, TouchEvent};
use yew::prelude::*;

/// Two taps closer than this many milliseconds form a double tap.
const DOUBLE_TAP_MS: f64 = 300.0;

/// Two taps closer than this many pixels (squared) form a double tap.
const DOUBLE_TAP_DIST_SQ: f64 = 900.0;

/// Callbacks for the touch events of the game view.
pub(super) struct TouchHandlers {
    pub(super) start: Callback<TouchEvent>,
    pub(super) moved: Callback<TouchEvent>,
    pub(super) end: Callback<TouchEvent>,
}

/// Configuration and state touch handlers read.
#[derive(Clone)]
pub(super) struct TouchContext {
    pub(super) handles: ViewHandles,
    pub(super) reducer: UseReducerHandle<GameStateReducer>,
}

/// Returns the client position of a touch.
fn touch_pos(touch: &Touch) -> Vec2 {
    vec2(touch.client_x() as f64, touch.client_y() as f64)
}

/// Returns the first two touches of an event.
fn two_fingers(event: &TouchEvent) -> Option<(Vec2, Vec2)> {
    let touches = event.touches();
    if touches.length() != 2 {
        return None;
    }
    Some((touch_pos(&touches.get(0)?), touch_pos(&touches.get(1)?)))
}

impl TouchContext {
    /// Builds the touch callbacks on top of the pointer callbacks.
    pub(super) fn handlers(&self, pointer: &PointerHandlers) -> TouchHandlers {
        let (start_ctx, move_ctx, end_ctx) = (self.clone(), self.clone(), self.clone());
        let (on_start, on_move, on_end) = (
            pointer.start.clone(),
            pointer.moved.clone(),
            pointer.end.clone(),
        );
        TouchHandlers {
            start: Callback::from(move |event: TouchEvent| start_ctx.on_start(&event, &on_start)),
            moved: Callback::from(move |event: TouchEvent| move_ctx.on_move(&event, &on_move)),
            end: Callback::from(move |event: TouchEvent| end_ctx.on_end(&event, &on_end)),
        }
    }

    /// Returns false when the event belongs to overlay UI or input is disabled.
    fn accepts(&self, event: &TouchEvent) -> bool {
        if is_ui_exempt_target(event.target()) {
            return false;
        }
        event.prevent_default();
        !self.reducer.has_match_result()
    }

    /// Begins a pinch with two fingers or a pointer press with one.
    fn on_start(&self, event: &TouchEvent, on_start: &Callback<InputStart>) {
        if !self.accepts(event) {
            return;
        }
        if let Some((a, b)) = two_fingers(event) {
            *self.handles.touch_gesture_active.borrow_mut() = true;
            self.handles.camera.borrow_mut().begin_pinch(a, b);
            self.handles.drag_start.set(None);
            self.handles.cancel_hold();
            self.handles.mark_not_dragging();
        } else if let Some(touch) = event.touches().get(0) {
            *self.handles.touch_gesture_active.borrow_mut() = false;
            on_start.emit(InputStart {
                pos: touch_pos(&touch),
                is_right_click: false,
            });
            self.forget_pinch();
        }
    }

    /// Pinch-zooms and pans with two fingers, or forwards single-finger movement.
    fn on_move(&self, event: &TouchEvent, on_move: &Callback<InputMove>) {
        if !self.accepts(event) {
            return;
        }
        if let Some((a, b)) = two_fingers(event) {
            *self.handles.touch_gesture_active.borrow_mut() = true;
            self.pinch(a, b);
            self.handles.mark_not_dragging();
        } else if let Some(touch) = event.touches().get(0) {
            let pos = touch_pos(&touch);
            if let Some(start) = *self.handles.drag_start
                && start.allow_panning
                && (pos - start.pos).length() > TAP_SLOP_PX
            {
                *self.handles.touch_gesture_active.borrow_mut() = true;
            }
            self.forget_pinch();
            on_move.emit(InputMove { pos });
        }
    }

    /// Ends a touch: finishes gestures, detects double taps, or forwards a tap.
    fn on_end(&self, event: &TouchEvent, on_end: &Callback<InputEnd>) {
        if is_ui_exempt_target(event.target()) {
            return;
        }
        event.prevent_default();
        self.forget_pinch();
        self.handles.drag_start.set(None);
        self.handles.mark_not_dragging();

        let fingers_left = event.touches().length();
        {
            let mut gesture = self.handles.touch_gesture_active.borrow_mut();
            if *gesture {
                if fingers_left == 0 {
                    *gesture = false;
                }
                return;
            }
        }
        let Some(touch) = event.changed_touches().get(0) else {
            return;
        };
        let pos = touch_pos(&touch);
        if fingers_left == 0 && !self.handles.is_dragging_piece() && self.is_double_tap(pos) {
            request_fullscreen();
            return;
        }
        on_end.emit(InputEnd {
            pos,
            is_right_click: false,
        });
    }

    /// Records a tap and returns true when it completes a double tap.
    fn is_double_tap(&self, pos: Vec2) -> bool {
        let now = performance_now();
        let mut last = self.handles.last_tap.borrow_mut();
        let is_double = last.is_some_and(|previous| {
            now - previous.time_ms <= DOUBLE_TAP_MS
                && (pos - previous.pos).length_squared() <= DOUBLE_TAP_DIST_SQ
        });
        *last = (!is_double).then_some(LastTap { time_ms: now, pos });
        is_double
    }

    /// Applies one pinch step with fingers at `a` and `b`, and shows where it moved the camera.
    fn pinch(&self, a: Vec2, b: Vec2) {
        let canvas = self.handles.canvas_ref.cast::<HtmlCanvasElement>();
        let mut camera = self.handles.camera.borrow_mut();
        camera.pinch_to(
            a,
            b,
            canvas.as_ref(),
            self.reducer.camera_movement_disabled(),
            self.reducer.board_rotated_180(),
        );
        let (position, zoom) = (camera.position(), camera.zoom());
        drop(camera);
        self.handles.cam_state.set(position);
        self.handles.zoom_state.set(zoom);
    }

    /// Clears pinch tracking.
    fn forget_pinch(&self) {
        self.handles.camera.borrow_mut().end_pinch();
    }
}
