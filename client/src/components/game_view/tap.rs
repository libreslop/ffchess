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
    /// Otherwise taps select your pieces at their predicted squares, and with a piece selected,
    /// tapping a reachable square queues a move from the piece's predicted square.
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
        let own_piece_at_target = predicted
            .piece_at(target)
            .filter(|piece| piece.owner_id == Some(self.player_id));
        let Some(selected) = self.selected else {
            return own_piece_at_target.map_or(TapOutcome::Ignore, |piece| {
                TapOutcome::Select(Some(piece.id))
            });
        };
        let Some(selected_piece) = predicted.pieces.get(&selected) else {
            return TapOutcome::Ignore;
        };

        if target == selected_piece.position {
            TapOutcome::Select(None)
        } else if let Some(piece) = own_piece_at_target {
            TapOutcome::Select(Some(piece.id))
        } else if self.can_move_to(selected, target) {
            TapOutcome::Move {
                piece_id: selected,
                target,
            }
        } else if predicted.piece_at(target).is_none() {
            TapOutcome::Select(None)
        } else {
            TapOutcome::Ignore
        }
    }

    /// Returns true when tapping `target` should not start panning the camera.
    pub(super) fn is_interactive(&self, target: BoardCoord) -> bool {
        self.projection.state.board_size.contains(target)
            && (self.projection.state.piece_at(target).is_some()
                || self
                    .selected
                    .is_some_and(|selected| self.can_move_to(selected, target)))
    }

    /// Returns true when `piece_id` may move to `target` from its predicted square.
    fn can_move_to(&self, piece_id: PieceId, target: BoardCoord) -> bool {
        let predicted = &self.projection.state;
        predicted.pieces.get(&piece_id).is_some_and(|piece| {
            self.catalog.piece(&piece.piece_type).is_ok_and(|config| {
                config
                    .check_move(predicted, piece.position, target, piece.owner_id)
                    .is_ok()
            })
        })
    }
}
