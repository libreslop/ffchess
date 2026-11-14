//! Mouse and single-pointer input: panning the camera, tapping squares, and dragging pieces.

use super::geometry::{grid_square, screen_to_grid_point};
use super::tap::{TapContext, TapOutcome};
use super::types::{
    DragStart, HeldPiece, InputEnd, InputMove, InputStart, TAP_SLOP_PX, ViewHandles,
};
use crate::math::Vec2;
use crate::premoves::Projection;
use crate::reducer::{GameAction, GameStateReducer, MsgSender};
use common::models::PieceAction;
use common::types::{BoardCoord, PieceId};
use std::rc::Rc;
use web_sys::HtmlCanvasElement;
use yew::prelude::*;

/// Everything pointer handlers need from the current render.
#[derive(Clone)]
pub(super) struct PointerContext {
    pub(super) handles: ViewHandles,
    pub(super) reducer: UseReducerHandle<GameStateReducer>,
    pub(super) tx: MsgSender,
    pub(super) projection: Rc<Projection>,
    pub(super) tile_size_px: f64,
    pub(super) drag_snap_squares: f64,
}

/// Callbacks for pointer presses, moves, and releases.
pub(super) struct PointerHandlers {
    pub(super) start: Callback<InputStart>,
    pub(super) moved: Callback<InputMove>,
    pub(super) end: Callback<InputEnd>,
}

impl PointerContext {
    /// Builds the pointer callbacks.
    pub(super) fn handlers(&self) -> PointerHandlers {
        let start = self.clone();
        let moved = self.clone();
        let end = self.clone();
        PointerHandlers {
            start: Callback::from(move |input| start.on_start(input)),
            moved: Callback::from(move |input| moved.on_move(input)),
            end: Callback::from(move |input| end.on_end(input)),
        }
    }

    /// Returns the CSS cursor for the board: grabbing while holding a piece, all-scroll while
    /// panning, pointer over squares a tap moves the selected piece to, and grab over pieces
    /// you can pick up.
    pub(super) fn cursor(&self) -> &'static str {
        if self.is_blocked() {
            return "default";
        }
        if self.handles.held.borrow().is_some() {
            return "grabbing";
        }
        let is_panning = self
            .handles
            .drag_start
            .is_some_and(|start| start.allow_panning)
            && *self.handles.did_pan.borrow();
        if is_panning {
            return "all-scroll";
        }
        let hovered = self.handles.camera.borrow().mouse_pos;
        let Some(square) = self.square_at(hovered) else {
            return "default";
        };
        let context = self.tap_context(*self.handles.selected);
        if context.is_move_target(square) {
            "pointer"
        } else if context.pickup_at(square).is_some() {
            "grab"
        } else {
            "default"
        }
    }

    /// Returns true while input must be ignored: after the match or during a scripted pan.
    fn is_blocked(&self) -> bool {
        self.reducer.has_match_result() || self.handles.camera.borrow().input_locked
    }

    /// Returns the board square under a screen position.
    fn square_at(&self, pos: Vec2) -> Option<BoardCoord> {
        self.grid_point_at(pos).map(grid_square)
    }

    /// Returns a screen position in grid units.
    fn grid_point_at(&self, pos: Vec2) -> Option<Vec2> {
        let canvas = self.handles.canvas_ref.cast::<HtmlCanvasElement>()?;
        let camera = self.handles.camera.borrow();
        Some(screen_to_grid_point(
            pos,
            &canvas,
            camera.camera,
            self.tile_size_px * camera.zoom,
            self.reducer.board_rotated_180(),
        ))
    }

    /// Builds the tap interpreter for the current state, with `selected` selected.
    fn tap_context(&self, selected: Option<PieceId>) -> TapContext<'_> {
        self.tap_context_with(&self.projection, selected)
    }

    /// Builds the tap interpreter against `projection`, with `selected` selected.
    fn tap_context_with<'a>(
        &'a self,
        projection: &'a Projection,
        selected: Option<PieceId>,
    ) -> TapContext<'a> {
        TapContext {
            state: &self.reducer.state,
            projection,
            premoves: &self.reducer.premoves,
            catalog: &self.reducer.catalog,
            player_id: self.reducer.viewer_id(),
            selected,
            now: self.reducer.server_now(),
        }
    }

    /// Starts a press; presses on pieces or move targets never pan the camera.
    ///
    /// A press on one of your pieces, either where queued actions will move it or where it
    /// stands now, picks it up and selects it, showing its move hints.
    fn on_start(&self, input: InputStart) {
        if self.is_blocked() {
            return;
        }
        *self.handles.did_pan.borrow_mut() = false;
        let selected = *self.handles.selected;
        let context = self.tap_context(selected);
        let square = self.square_at(input.pos).filter(|_| !input.is_right_click);
        let is_interactive = square.is_some_and(|square| context.is_interactive(square));
        let held = square.and_then(|square| context.pickup_at(square)).map(
            |(piece_id, replaces_actions)| {
                HeldPiece::new(piece_id, input.pos, selected, replaces_actions)
            },
        );
        if let Some(held) = held {
            self.handles.selected.set(Some(held.piece_id));
        }
        *self.handles.held.borrow_mut() = held;
        self.handles.drag_start.set(Some(DragStart {
            pos: input.pos,
            allow_panning: !is_interactive && !self.reducer.camera_movement_disabled(),
        }));
        self.handles.camera.borrow_mut().velocity = Vec2::ZERO;
    }

    /// Pans the camera while a panning press moves.
    fn on_move(&self, input: InputMove) {
        if self.is_blocked() {
            return;
        }
        if let Some(held) = self.handles.held.borrow_mut().as_mut() {
            held.follow(input.pos);
        }
        let mut camera = self.handles.camera.borrow_mut();
        camera.mouse_pos = input.pos;
        let Some(start) = *self.handles.drag_start else {
            return;
        };
        if !start.allow_panning {
            return;
        }
        if self.reducer.camera_movement_disabled() {
            camera.velocity = Vec2::ZERO;
            return;
        }
        let delta = input.pos - start.pos;
        if delta.abs().max_element() <= 0.1 {
            return;
        }

        *self.handles.did_pan.borrow_mut() = true;
        let movement = if self.reducer.board_rotated_180() {
            delta
        } else {
            -delta
        };
        camera.camera += movement;
        camera.velocity = movement;
        let is_alive = self
            .reducer
            .active_player_id()
            .is_some_and(|id| self.reducer.state.players.contains_key(&id));
        if !is_alive {
            camera.target_camera = camera.camera;
        }
        self.handles.cam_state.set(camera.camera);
        self.handles.drag_start.set(Some(DragStart {
            pos: input.pos,
            allow_panning: true,
        }));
    }

    /// Ends a press: drops a dragged piece, or treats the press as a tap when it did not pan.
    fn on_end(&self, input: InputEnd) {
        let held = self.handles.held.borrow_mut().take();
        if self.is_blocked() {
            self.handles.cancel_drag();
            return;
        }
        let start = *self.handles.drag_start;
        self.handles.drag_start.set(None);
        let panned = std::mem::take(&mut *self.handles.did_pan.borrow_mut());
        let is_tap = match start {
            Some(start) => {
                if !start.allow_panning {
                    self.handles.camera.borrow_mut().velocity = Vec2::ZERO;
                }
                let dragged = start.allow_panning && (input.pos - start.pos).length() > TAP_SLOP_PX;
                !panned && !dragged
            }
            None => {
                self.handles.camera.borrow_mut().velocity = Vec2::ZERO;
                true
            }
        };
        if let Some(held) = held.filter(|held| held.dragged) {
            self.drop_piece(held, input.pos);
            return;
        }
        if !is_tap {
            return;
        }
        if input.is_right_click {
            self.handles.selected.set(None);
            return;
        }
        // A tap on a held piece is resolved against the selection from before the press.
        let selected = match held {
            Some(held) => {
                self.handles.selected.set(held.previous_selected);
                held.previous_selected
            }
            None => *self.handles.selected,
        };
        if let Some(square) = self.square_at(input.pos) {
            let outcome = self.tap_context(selected).resolve(square);
            self.apply_tap(outcome);
        }
    }

    /// Drops a dragged piece at `pos`, moving it there as if that square was tapped with the
    /// piece selected, or to a legal square close enough to snap to; other drops leave it
    /// selected in place.
    ///
    /// A piece picked up from where it stands now moves from there, replacing its queued
    /// actions.
    fn drop_piece(&self, held: HeldPiece, pos: Vec2) {
        let Some(point) = self.grid_point_at(pos) else {
            return;
        };
        let piece_id = held.piece_id;
        let replaced = held.replaces_actions.then(|| {
            self.reducer
                .premoves
                .without_piece(piece_id)
                .project(&self.reducer.state, &self.reducer.catalog)
        });
        let projection = replaced.as_ref().unwrap_or(&self.projection);
        let Some(target) = self
            .tap_context_with(projection, Some(piece_id))
            .drop_target(piece_id, point, self.drag_snap_squares)
        else {
            return;
        };
        if held.replaces_actions {
            self.reducer.dispatch(GameAction::ClearActions {
                piece_id,
                tx: self.tx.clone(),
            });
        }
        self.apply_tap(TapOutcome::Move { piece_id, target });
    }

    /// Carries out the outcome of a tap.
    fn apply_tap(&self, outcome: TapOutcome) {
        match outcome {
            TapOutcome::Select(piece_id) => self.handles.selected.set(piece_id),
            TapOutcome::ClearActions { piece_id, select } => {
                self.reducer.dispatch(GameAction::ClearActions {
                    piece_id,
                    tx: self.tx.clone(),
                });
                self.handles.selected.set(select);
            }
            TapOutcome::Move { piece_id, target } => {
                self.reducer.dispatch(GameAction::QueueAction {
                    piece_id,
                    action: PieceAction::Move { target },
                    tx: self.tx.clone(),
                });
            }
            TapOutcome::Ignore => {}
        }
    }
}

/// Builds the handler that stops panning when the cursor leaves the view.
///
/// The camera keeps its inertia but stops easing back towards an old target.
pub(super) fn mouse_leave_handler(handles: ViewHandles) -> Callback<MouseEvent> {
    Callback::from(move |_| {
        let previous = *handles.drag_start;
        handles.cancel_hold();
        handles.drag_start.set(None);
        *handles.did_pan.borrow_mut() = false;
        if previous.is_some_and(|drag| drag.allow_panning) {
            let mut camera = handles.camera.borrow_mut();
            camera.target_camera = camera.camera;
        }
    })
}
