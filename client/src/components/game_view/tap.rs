//! Decides what a tap on the board means for selection and queued actions.

use crate::premoves::{PremoveQueue, Projection};
use common::models::GameState;
use common::rules::Catalog;
use common::types::{BoardCoord, PieceId, PlayerId};

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

    /// Resolves dropping the dragged `piece_id` on `target`: the move tapping `target` would
    /// queue with the piece selected, or nothing when it selects no legal move.
    pub(super) fn resolve_drop(&self, piece_id: PieceId, target: BoardCoord) -> TapOutcome {
        self.move_target(piece_id, target)
            .map_or(TapOutcome::Ignore, |target| TapOutcome::Move { piece_id, target })
    }

    /// Returns the local player's piece predicted to stand on `target`, which a press there
    /// picks up.
    pub(super) fn own_piece_at(&self, target: BoardCoord) -> Option<PieceId> {
        self.projection
            .state
            .piece_at(target)
            .filter(|piece| piece.owner_id == Some(self.player_id))
            .map(|piece| piece.id)
    }

    /// Returns true when tapping `target` should not start panning the camera.
    pub(super) fn is_interactive(&self, target: BoardCoord) -> bool {
        self.projection.state.board_size.contains(target)
            && (self.projection.state.piece_at(target).is_some() || self.is_move_target(target))
    }

    /// Returns true when tapping `target` selects a legal move of the selected piece.
    pub(super) fn is_move_target(&self, target: BoardCoord) -> bool {
        self.selected
            .is_some_and(|selected| self.move_target(selected, target).is_some())
    }

    /// Returns where `piece_id` lands when tapping `tapped` selects one of its legal moves
    /// from its predicted square.
    ///
    /// A move is selected by its hint squares (see [`common::rules::LegalMove::handles`]); a
    /// move landing on the tapped square wins over one that only affects the piece there.
    fn move_target(&self, piece_id: PieceId, tapped: BoardCoord) -> Option<BoardCoord> {
        let predicted = &self.projection.state;
        let piece = predicted.pieces.get(&piece_id)?;
        self.catalog
            .legal_moves(predicted, piece)
            .into_iter()
            .filter(|legal| legal.handles().contains(&tapped))
            .min_by_key(|legal| legal.target != tapped)
            .map(|legal| legal.target)
    }
}
