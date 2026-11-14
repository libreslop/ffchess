//! Decides what a tap on the board means for selection and queued actions.

use super::geometry::grid_square;
use crate::math::Vec2;
use crate::premoves::{PremoveQueue, Projection};
use common::models::GameState;
use common::rules::Catalog;
use common::types::{BoardCoord, PieceId, PlayerId, TimestampMs};

/// What a tap on a board square should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TapOutcome {
    /// Change the selected piece (or clear the selection).
    Select(Option<PieceId>),
    /// Drop the queued actions of a piece, then change the selection.
    ClearActions {
        piece_id: PieceId,
        select: Option<PieceId>,
    },
    /// Queue a move of the selected piece to the tapped square.
    Move {
        piece_id: PieceId,
        target: BoardCoord,
    },
    /// Leave everything as it is.
    Ignore,
}

/// Everything needed to interpret a tap.
pub(super) struct TapContext<'a> {
    pub(super) state: &'a GameState,
    pub(super) projection: &'a Projection,
    pub(super) premoves: &'a PremoveQueue,
    pub(super) catalog: &'a Catalog,
    pub(super) player_id: PlayerId,
    pub(super) selected: Option<PieceId>,
    /// Server time, deciding whether a move runs at once or is queued.
    pub(super) now: TimestampMs,
}

impl TapContext<'_> {
    /// Resolves a tap on `target`.
    ///
    /// Tapping one of your pieces that has queued actions (where it stands now) clears them.
    /// Otherwise, with a piece selected, tapping a square that selects one of its legal moves
    /// queues that move from the piece's predicted square, even when another of your pieces
    /// stands there (the rook when castling). Taps on your pieces that select no move select
    /// them.
    pub(super) fn resolve(&self, target: BoardCoord) -> TapOutcome {
        if let Some(piece) = self.state.piece_at(target).filter(|piece| {
            piece.owner_id == Some(self.player_id) && self.premoves.has_actions_for(piece.id)
        }) {
            let select = (self.selected != Some(piece.id)).then_some(piece.id);
            return TapOutcome::ClearActions {
                piece_id: piece.id,
                select,
            };
        }

        let predicted = &self.projection.state;
        let own_piece_at_target = self.own_piece_at(target);
        let Some(selected) = self.selected else {
            return own_piece_at_target.map_or(TapOutcome::Ignore, |piece_id| {
                TapOutcome::Select(Some(piece_id))
            });
        };
        let Some(selected_piece) = predicted.pieces.get(&selected) else {
            return TapOutcome::Ignore;
        };

        if target == selected_piece.position {
            TapOutcome::Select(None)
        } else if let Some(move_target) = self.move_target(selected, target) {
            TapOutcome::Move {
                piece_id: selected,
                target: move_target,
            }
        } else if let Some(piece_id) = own_piece_at_target {
            TapOutcome::Select(Some(piece_id))
        } else if predicted.piece_at(target).is_none() {
            TapOutcome::Select(None)
        } else {
            TapOutcome::Ignore
        }
    }

    /// Returns where the dragged `piece_id` lands when dropped at `point`, in grid units: the
    /// move tapping the square under `point` would queue with the piece selected.
    ///
    /// When that square selects no legal move, the drop snaps to the nearest square that
    /// does, if one is within `snap_squares` of `point`.
    pub(super) fn drop_target(
        &self,
        piece_id: PieceId,
        point: Vec2,
        snap_squares: f64,
    ) -> Option<BoardCoord> {
        let square = grid_square(point);
        if let Some(target) = self.move_target(piece_id, square) {
            return Some(target);
        }
        let reach = snap_squares.max(0.0).ceil() as i32;
        (-reach..=reach)
            .flat_map(|dx| (-reach..=reach).map(move |dy| BoardCoord::new(dx, dy)))
            .map(|offset| BoardCoord(square.0 + offset.0))
            .map(|candidate| (candidate, distance_to_square(point, candidate)))
            .filter(|(_, distance)| *distance <= snap_squares)
            .filter_map(|(candidate, distance)| {
                Some((self.move_target(piece_id, candidate)?, distance))
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(target, _)| target)
    }

    /// Returns the local player's piece a press on `target` picks up, and whether it is picked
    /// up from where it stands now rather than from where its queued actions will move it.
    ///
    /// Pieces where queued actions will move them win over pieces standing there now.
    pub(super) fn pickup_at(&self, target: BoardCoord) -> Option<(PieceId, bool)> {
        self.own_piece_at(target)
            .map(|piece_id| (piece_id, false))
            .or_else(|| {
                self.own_queued_piece_at(target)
                    .map(|piece_id| (piece_id, true))
            })
    }

    /// Returns the local player's piece predicted to stand on `target`.
    pub(super) fn own_piece_at(&self, target: BoardCoord) -> Option<PieceId> {
        self.projection
            .state
            .piece_at(target)
            .filter(|piece| piece.owner_id == Some(self.player_id))
            .map(|piece| piece.id)
    }

    /// Returns the local player's piece with queued actions standing on `target` now.
    fn own_queued_piece_at(&self, target: BoardCoord) -> Option<PieceId> {
        self.state
            .piece_at(target)
            .filter(|piece| {
                piece.owner_id == Some(self.player_id) && self.premoves.has_actions_for(piece.id)
            })
            .map(|piece| piece.id)
    }

    /// Returns true when tapping `target` should not start panning the camera.
    pub(super) fn is_interactive(&self, target: BoardCoord) -> bool {
        self.projection.state.board_size.contains(target)
            && (self.projection.state.piece_at(target).is_some()
                || self.own_queued_piece_at(target).is_some()
                || self.is_move_target(target))
    }

    /// Returns true when tapping `target` selects a legal move of the selected piece.
    pub(super) fn is_move_target(&self, target: BoardCoord) -> bool {
        self.selected
            .is_some_and(|selected| self.move_target(selected, target).is_some())
    }

    /// Returns where `piece_id` lands when tapping `tapped` selects one of its legal moves
    /// from its predicted square, checked as a premove when it would be queued.
    ///
    /// A move is selected by its hint squares (see [`common::rules::LegalMove::handles`]); a
    /// move landing on the tapped square wins over one that only affects the piece there.
    fn move_target(&self, piece_id: PieceId, tapped: BoardCoord) -> Option<BoardCoord> {
        let predicted = &self.projection.state;
        let piece = predicted.pieces.get(&piece_id)?;
        self.catalog
            .legal_moves(
                predicted,
                piece,
                self.projection.move_timing(piece, self.now),
            )
            .into_iter()
            .filter(|legal| legal.handles().contains(&tapped))
            .min_by_key(|legal| legal.target != tapped)
            .map(|legal| legal.target)
    }
}

/// Returns the distance, in grid units, from `point` to the nearest point of `square`.
fn distance_to_square(point: Vec2, square: BoardCoord) -> f64 {
    let min = square.0.as_dvec2();
    (min - point)
        .max(point - (min + 1.0))
        .max(Vec2::ZERO)
        .length()
}
