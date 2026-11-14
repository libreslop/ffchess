//! Per-piece queues of player actions and their execution.
//!
//! Every [`ActionRequest`] receives exactly one [`ServerMessage::ActionResult`]. Actions of one
//! piece run strictly in order: moves wait for the piece cooldown, purchases run as soon as
//! everything before them has resolved. When an action fails, the rest of its piece queue is
//! cancelled, and so is any later request that names a failed action as its predecessor.

use super::world::World;
use common::models::{MoveTiming, PieceAction};
use common::protocol::{ActionRequest, GameError, ServerMessage};
use common::rules::Simulation;
use common::types::{ActionId, BoardCoord, PieceId, PlayerId, TimestampMs};
use std::collections::{HashMap, HashSet, VecDeque};

/// Maximum number of actions waiting on one piece.
const MAX_QUEUED_PER_PIECE: usize = 100;

/// Number of failed action ids remembered for chain cancellation.
const CANCELLED_MEMORY: usize = 1024;

/// A player action waiting for its piece.
#[derive(Debug, Clone, Copy)]
pub(super) struct QueuedAction {
    player_id: PlayerId,
    id: ActionId,
    action: PieceAction,
}

/// Bounded memory of failed or cleared actions, used to cancel chains planned after them.
#[derive(Default)]
struct CancelledActions {
    order: VecDeque<(PlayerId, ActionId)>,
    ids: HashSet<(PlayerId, ActionId)>,
}

impl CancelledActions {
    /// Remembers that an action did not complete.
    fn insert(&mut self, player_id: PlayerId, id: ActionId) {
        if self.ids.insert((player_id, id)) {
            self.order.push_back((player_id, id));
        }
        while self.order.len() > CANCELLED_MEMORY {
            if let Some(oldest) = self.order.pop_front() {
                self.ids.remove(&oldest);
            }
        }
    }

    /// Returns true when the action was recorded as not completed.
    fn contains(&self, player_id: PlayerId, id: ActionId) -> bool {
        self.ids.contains(&(player_id, id))
    }

    /// Forgets every action of one player.
    fn forget_player(&mut self, player_id: PlayerId) {
        self.order.retain(|(owner, _)| *owner != player_id);
        self.ids.retain(|(owner, _)| *owner != player_id);
    }
}

/// Queues of pending actions keyed by the piece they act on.
#[derive(Default)]
pub(super) struct ActionQueues {
    queues: HashMap<PieceId, VecDeque<QueuedAction>>,
    cancelled: CancelledActions,
}

impl ActionQueues {
    /// Returns true when `piece_id` has actions waiting.
    fn is_busy(&self, piece_id: PieceId) -> bool {
        self.queues.contains_key(&piece_id)
    }

    /// Appends an action to the queue of `piece_id`; returns it back when the queue is full.
    fn push(&mut self, piece_id: PieceId, queued: QueuedAction) -> Result<(), QueuedAction> {
        let queue = self.queues.entry(piece_id).or_default();
        if queue.len() >= MAX_QUEUED_PER_PIECE {
            return Err(queued);
        }
        queue.push_back(queued);
        Ok(())
    }

    /// Returns the next action of `piece_id`.
    fn front(&self, piece_id: PieceId) -> Option<QueuedAction> {
        self.queues.get(&piece_id)?.front().copied()
    }

    /// Removes the next action of `piece_id`, dropping the queue once empty.
    fn pop_front(&mut self, piece_id: PieceId) {
        if let Some(queue) = self.queues.get_mut(&piece_id) {
            queue.pop_front();
            if queue.is_empty() {
                self.queues.remove(&piece_id);
            }
        }
    }

    /// Removes and returns the whole queue of `piece_id`.
    fn take(&mut self, piece_id: PieceId) -> VecDeque<QueuedAction> {
        self.queues.remove(&piece_id).unwrap_or_default()
    }

    /// Returns the pieces that have actions waiting.
    fn busy_pieces(&self) -> Vec<PieceId> {
        self.queues.keys().copied().collect()
    }

    /// Forgets the failed actions of a player starting a new session.
    ///
    /// Action ids are only unique per connection, so a returning player may reuse old ids.
    pub(super) fn forget_player(&mut self, player_id: PlayerId) {
        self.cancelled.forget_player(player_id);
    }
}

impl World {
    /// Accepts an action request from `player_id`, running it now when nothing blocks it.
    pub(super) fn submit_action(
        &mut self,
        player_id: PlayerId,
        request: ActionRequest,
        now: TimestampMs,
    ) {
        let queued = QueuedAction {
            player_id,
            id: request.id,
            action: request.action,
        };
        let predecessor_failed = request
            .after
            .is_some_and(|after| self.actions.cancelled.contains(player_id, after));
        if predecessor_failed {
            return self.finish_action(queued, Err(GameError::ActionCancelled));
        }
        if let Err(error) = self.check_ownership(player_id, request.piece_id) {
            return self.finish_action(queued, Err(error));
        }

        if !self.actions.is_busy(request.piece_id)
            && self.is_action_ready(request.piece_id, request.action, now)
        {
            self.run_action(request.piece_id, queued, now);
        } else if let Err(rejected) = self.actions.push(request.piece_id, queued) {
            let error = GameError::Internal("Too many queued actions".to_string());
            self.finish_action(rejected, Err(error));
        }
    }

    /// Drops every queued action `player_id` has on `piece_id`.
    pub(super) fn clear_actions(&mut self, player_id: PlayerId, piece_id: PieceId) {
        if self.check_ownership(player_id, piece_id).is_ok() {
            self.cancel_queue(piece_id);
        }
    }

    /// Runs every queued action whose piece is ready, in queue order.
    pub(super) fn process_actions(&mut self, now: TimestampMs) {
        for piece_id in self.actions.busy_pieces() {
            while let Some(next) = self.actions.front(piece_id) {
                if !self.is_action_ready(piece_id, next.action, now) {
                    break;
                }
                self.actions.pop_front(piece_id);
                if !self.run_action(piece_id, next, now) {
                    self.cancel_queue(piece_id);
                    break;
                }
            }
        }
    }

    /// Cancels the queues of pieces that were disturbed or left the board.
    ///
    /// Actions of pieces that left the board fail with [`GameError::PieceNotFound`], so their
    /// players can tell them apart from rejected actions of pieces still alive.
    pub(super) fn cancel_actions_of(&mut self, piece_ids: impl IntoIterator<Item = PieceId>) {
        for piece_id in piece_ids {
            let error = if self.state.pieces.contains_key(&piece_id) {
                GameError::ActionCancelled
            } else {
                GameError::PieceNotFound
            };
            self.cancel_queue_with(piece_id, error);
        }
    }

    /// Cancels every queued action of `piece_id`.
    fn cancel_queue(&mut self, piece_id: PieceId) {
        self.cancel_queue_with(piece_id, GameError::ActionCancelled);
    }

    /// Fails every queued action of `piece_id` with `error`.
    fn cancel_queue_with(&mut self, piece_id: PieceId, error: GameError) {
        for queued in self.actions.take(piece_id) {
            self.finish_action(queued, Err(error.clone()));
        }
    }

    /// Records the outcome of an action and reports it to its player.
    fn finish_action(&mut self, queued: QueuedAction, result: Result<(), GameError>) {
        self.report_action(queued, result, Vec::new());
    }

    /// Records the outcome of an action and reports it to its player, together with the
    /// squares to blame when it failed.
    fn report_action(
        &mut self,
        queued: QueuedAction,
        result: Result<(), GameError>,
        culprits: Vec<BoardCoord>,
    ) {
        if result.is_err() {
            self.actions.cancelled.insert(queued.player_id, queued.id);
        }
        self.outbox.send_to_player(
            queued.player_id,
            ServerMessage::ActionResult {
                id: queued.id,
                result,
                culprits,
            },
        );
    }

    /// Returns true when `action` on `piece_id` may run at `now`.
    ///
    /// Purchases never wait; moves wait for the match countdown and the piece cooldown.
    fn is_action_ready(&self, piece_id: PieceId, action: PieceAction, now: TimestampMs) -> bool {
        if !action.needs_cooldown() {
            return true;
        }
        !self.clock.moves_locked(now)
            && self
                .state
                .pieces
                .get(&piece_id)
                .is_none_or(|piece| piece.is_ready(now))
    }

    /// Checks that `piece_id` exists and belongs to `player_id`.
    fn check_ownership(&self, player_id: PlayerId, piece_id: PieceId) -> Result<(), GameError> {
        let piece = self
            .state
            .pieces
            .get(&piece_id)
            .ok_or(GameError::PieceNotFound)?;
        if piece.owner_id == Some(player_id) {
            Ok(())
        } else {
            Err(GameError::NotYourPiece)
        }
    }

    /// Applies one action to the live board and reports its outcome.
    ///
    /// Returns true when the action succeeded.
    fn run_action(&mut self, piece_id: PieceId, queued: QueuedAction, now: TimestampMs) -> bool {
        let result = self.execute_action(piece_id, queued, now);
        // A failed action leaves the board untouched, so the move is explained as it stands.
        let culprits = match (&result, queued.action, self.state.pieces.get(&piece_id)) {
            (Err(_), PieceAction::Move { target }, Some(piece)) => self
                .setup
                .catalog
                .move_culprits(&self.state, piece, target, MoveTiming::Now),
            _ => Vec::new(),
        };
        let succeeded = result.is_ok();
        self.report_action(queued, result, culprits);
        succeeded
    }

    /// Applies one action to the live board.
    fn execute_action(
        &mut self,
        piece_id: PieceId,
        queued: QueuedAction,
        now: TimestampMs,
    ) -> Result<(), GameError> {
        self.check_ownership(queued.player_id, piece_id)?;
        let catalog = self.setup.catalog.clone();
        let effects =
            Simulation::new(&catalog, &mut self.state, now).apply(piece_id, queued.action)?;
        self.apply_effects(Some(queued.player_id), effects);
        Ok(())
    }
}
