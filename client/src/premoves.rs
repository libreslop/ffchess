//! Client-side record of piece actions sent to the server but not yet visible in snapshots.
//!
//! Every action is sent to the server as soon as it is queued here. The server answers each
//! one with exactly one result: a success marks it confirmed and it is dropped once the next
//! snapshot (which already contains it) arrives; a failure drops it together with every later
//! action of the same piece, mirroring the server's chain cancellation.

use common::models::{GameState, MoveTiming, Piece, PieceAction};
use common::protocol::{ActionRequest, GameError};
use common::rules::{Catalog, PieceMove, Simulation};
use common::types::{ActionId, BoardCoord, PieceId, TimestampMs};
use std::collections::{HashMap, HashSet, VecDeque};

/// Number of failed action ids remembered for flashing their trails.
const FAILED_MEMORY: usize = 64;

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
    /// The queued action making the movement.
    pub action: ActionId,
    pub piece_id: PieceId,
    pub start: BoardCoord,
    pub target: BoardCoord,
}

impl PremoveLine {
    /// Builds the line of `moved`, relocated by the queued action `action`.
    fn new(action: ActionId, moved: PieceMove) -> Self {
        Self {
            action,
            piece_id: moved.piece_id,
            start: moved.from,
            target: moved.to,
        }
    }
}

/// A queued move that failed: `piece_id` could not move into `target`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FailedMove {
    pub piece_id: PieceId,
    pub target: BoardCoord,
}

impl FailedMove {
    /// Returns the failed move of `premove`, or `None` when it is not a move.
    fn of(premove: &Premove) -> Option<Self> {
        match premove.action {
            PieceAction::Move { target } => Some(Self {
                piece_id: premove.piece_id,
                target,
            }),
            PieceAction::Buy { .. } => None,
        }
    }
}

/// The board as it will look once every queued action has run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Projection {
    pub state: GameState,
    pub lines: Vec<PremoveLine>,
    /// Pieces with queued actions.
    pub queued: HashSet<PieceId>,
    /// Actions recently rejected by the server, together with the rest of their piece's
    /// chain. Each action that failed itself maps to its move; the actions cancelled after it
    /// map to `None`.
    pub failed: HashMap<ActionId, Option<FailedMove>>,
}

impl Projection {
    /// Returns when a new move of `piece`, as predicted, would run at `now`: at once when it
    /// has nothing queued and is off cooldown, otherwise as a premove.
    pub fn move_timing(&self, piece: &Piece, now: TimestampMs) -> MoveTiming {
        if self.queued.contains(&piece.id) || !piece.is_ready(now) {
            MoveTiming::Premove
        } else {
            MoveTiming::Now
        }
    }
}

/// Ordered queue of the local player's unresolved actions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PremoveQueue {
    items: Vec<Premove>,
    next_id: ActionId,
    /// Recently failed actions, oldest first, as in [`Projection::failed`].
    failed: VecDeque<(ActionId, Option<FailedMove>)>,
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
                // The piece left the board, so its trail vanishes without flashing.
                let piece_gone = error == GameError::PieceNotFound;
                let mut position = 0;
                let mut dropped = Vec::new();
                self.items.retain(|premove| {
                    let keep = position < index || premove.piece_id != piece_id;
                    position += 1;
                    if !keep && !piece_gone {
                        let failed_move = (premove.id == id).then(|| FailedMove::of(premove));
                        dropped.push((premove.id, failed_move.flatten()));
                    }
                    keep
                });
                self.failed.extend(dropped);
                let excess = self.failed.len().saturating_sub(FAILED_MEMORY);
                self.failed.drain(..excess);
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

    /// Returns a copy of the queue without the actions of one piece.
    pub fn without_piece(&self, piece_id: PieceId) -> Self {
        let mut queue = self.clone();
        queue.clear_piece(piece_id);
        queue
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
    /// Actions are checked as premoves, so requirements marked `ignore_for_premove` are
    /// skipped here; the server checks them when the action runs. A move predicted to fail
    /// anyway (another piece in the way) is still shown, moving the piece straight to its
    /// target, so the trail stays until the server rejects it.
    pub fn project(&self, state: &GameState, catalog: &Catalog) -> Projection {
        let mut projected = state.clone();
        let mut lines = Vec::new();
        let mut queued = HashSet::new();
        for premove in &self.items {
            queued.insert(premove.piece_id);
            // Move timestamps of predicted pieces are never shown, so any clock value works.
            let applied = Simulation::new(catalog, &mut projected, TimestampMs::default())
                .with_timing(MoveTiming::Premove)
                .apply(premove.piece_id, premove.action);
            match applied {
                Ok(effects) => lines.extend(
                    effects
                        .moved
                        .into_iter()
                        .map(|moved| PremoveLine::new(premove.id, moved)),
                ),
                Err(_) => lines.extend(
                    force_move(&mut projected, premove)
                        .map(|moved| PremoveLine::new(premove.id, moved)),
                ),
            }
        }
        Projection {
            state: projected,
            lines,
            queued,
            failed: self.failed.iter().copied().collect(),
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

/// Moves the piece of a queued move straight to its target, ignoring every rule, and
/// returns the movement; purchases and moves of missing pieces change nothing.
fn force_move(state: &mut GameState, premove: &Premove) -> Option<PieceMove> {
    let PieceAction::Move { target } = premove.action else {
        return None;
    };
    let piece = state.pieces.get_mut(&premove.piece_id)?;
    let from = std::mem::replace(&mut piece.position, target);
    Some(PieceMove {
        piece_id: premove.piece_id,
        from,
        to: target,
    })
}
