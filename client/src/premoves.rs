//! Client-side record of piece actions sent to the server but not yet visible in snapshots.
//!
//! Every action is sent to the server as soon as it is queued here. The server answers each
//! one with exactly one result: a success marks it confirmed and it is dropped once the next
//! snapshot (which already contains it) arrives; a failure drops it together with every later
//! action of the same piece, mirroring the server's chain cancellation.

use common::models::{GameState, PieceAction};
use common::protocol::{ActionRequest, GameError};
use common::rules::{Catalog, PieceMove, Simulation};
use common::types::{ActionId, BoardCoord, PieceId, TimestampMs};
use std::collections::HashSet;

/// Lifecycle of a queued action as known to the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PremoveStatus {
    /// Sent; the server has not reported a result yet.
    Pending,
    /// Applied by the server; the next snapshot will contain it.
    Confirmed,
}

/// One action the client is waiting to see in the authoritative state.
#[derive(Clone, Debug, PartialEq)]
struct Premove {
    id: ActionId,
    piece_id: PieceId,
    action: PieceAction,
    status: PremoveStatus,
}

/// A predicted movement drawn as an arrow from `start` to `target`.
///
/// A special move draws one line per piece it relocates, partners included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PremoveLine {
    pub piece_id: PieceId,
    pub start: BoardCoord,
    pub target: BoardCoord,
}

impl From<PieceMove> for PremoveLine {
    fn from(moved: PieceMove) -> Self {
        Self {
            piece_id: moved.piece_id,
            start: moved.from,
            target: moved.to,
        }
    }
}

/// The board as it will look once every queued action has run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Projection {
    pub state: GameState,
    pub lines: Vec<PremoveLine>,
}

/// Ordered queue of the local player's unresolved actions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PremoveQueue {
    items: Vec<Premove>,
    next_id: ActionId,
}

impl PremoveQueue {
    /// Creates the request for a new action of `piece_id`, chained after its last queued action.
    ///
    /// The action is recorded as pending; the caller sends the returned request.
    pub fn enqueue(&mut self, piece_id: PieceId, action: PieceAction) -> ActionRequest {
        self.next_id = self.next_id.next();
        let request = ActionRequest {
            id: self.next_id,
            piece_id,
            action,
            after: self.last_of(piece_id).map(|premove| premove.id),
        };
        self.items.push(Premove {
            id: request.id,
            piece_id,
            action,
            status: PremoveStatus::Pending,
        });
        request
    }

    /// Applies the server's result for an action.
    ///
    /// Returns the error to show the player when a known action failed.
    pub fn resolve(&mut self, id: ActionId, result: Result<(), GameError>) -> Option<GameError> {
        let index = self.items.iter().position(|premove| premove.id == id)?;
        match result {
            Ok(()) => {
                self.items[index].status = PremoveStatus::Confirmed;
                None
            }
            Err(error) => {
                let piece_id = self.items[index].piece_id;
                let mut position = 0;
                self.items.retain(|premove| {
                    let keep = position < index || premove.piece_id != piece_id;
                    position += 1;
                    keep
                });
                Some(error)
            }
        }
    }

    /// Reconciles the queue with a fresh authoritative snapshot.
    ///
    /// Confirmed actions are part of the snapshot now; actions of vanished pieces never run.
    pub fn sync_with(&mut self, state: &GameState) {
        self.items.retain(|premove| {
            premove.status == PremoveStatus::Pending && state.pieces.contains_key(&premove.piece_id)
        });
    }

    /// Forgets every queued action of one piece.
    pub fn clear_piece(&mut self, piece_id: PieceId) {
        self.items.retain(|premove| premove.piece_id != piece_id);
    }

    /// Forgets every queued action.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Returns true when `piece_id` has unresolved actions.
    pub fn has_actions_for(&self, piece_id: PieceId) -> bool {
        self.items
            .iter()
            .any(|premove| premove.piece_id == piece_id)
    }

    /// Returns true when the latest queued action of `piece_id` is a purchase.
    pub fn ends_with_purchase(&self, piece_id: PieceId) -> bool {
        self.last_of(piece_id)
            .is_some_and(|premove| matches!(premove.action, PieceAction::Buy { .. }))
    }

    /// Predicts the board after every queued action, in order.
    ///
    /// An action that would fail hides itself and the rest of its piece's chain, since the
    /// server will cancel them as well.
    pub fn project(&self, state: &GameState, catalog: &Catalog) -> Projection {
        let mut projected = state.clone();
        let mut lines = Vec::new();
        let mut broken_pieces = HashSet::new();
        for premove in &self.items {
            if broken_pieces.contains(&premove.piece_id) {
                continue;
            }
            // Move timestamps of predicted pieces are never shown, so any clock value works.
            let applied = Simulation::new(catalog, &mut projected, TimestampMs::default())
                .apply(premove.piece_id, premove.action);
            match applied {
                Ok(effects) => lines.extend(effects.moved.into_iter().map(PremoveLine::from)),
                Err(_) => {
                    broken_pieces.insert(premove.piece_id);
                }
            }
        }
        Projection {
            state: projected,
            lines,
        }
    }

    /// Returns the latest queued action of a piece.
    fn last_of(&self, piece_id: PieceId) -> Option<&Premove> {
        self.items
            .iter()
            .rev()
            .find(|premove| premove.piece_id == piece_id)
    }
}
