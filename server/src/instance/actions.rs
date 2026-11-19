//! Per-piece queues of player actions and their execution.
//!
//! Every [`ActionRequest`] receives exactly one [`ServerMessage::ActionResult`]. Actions of one
//! piece run strictly in order: moves wait for the piece cooldown, guards also wait until an
//! enemy takes the square they guard (and fail at once when it is left empty), and purchases
//! run as soon as everything before them has resolved. When an action fails, the rest of its piece queue is cancelled,
//! and so is any later request that names a failed action as its predecessor.
//!
//! Actions of different pieces that are ready at the same time run in the order their requests
//! arrived, so of several guards of one square, the one requested first takes it back.

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

/// Position of a request in the order requests arrived at the instance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Arrival(u64);

impl Arrival {
    /// Returns the position following this one.
    fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// A player action waiting for its piece.
#[derive(Debug, Clone, Copy)]
pub(super) struct QueuedAction {
    player_id: PlayerId,
    id: ActionId,
    action: PieceAction,
    arrival: Arrival,
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
    /// Arrival of the latest request.
    last_arrival: Arrival,
}

impl ActionQueues {
    /// Records the arrival of a request from `player_id`.
    fn arrive(&mut self, player_id: PlayerId, request: &ActionRequest) -> QueuedAction {
        self.last_arrival = self.last_arrival.next();
        QueuedAction {
            player_id,
            id: request.id,
            action: request.action,
            arrival: self.last_arrival,
        }
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

    /// Removes and returns the last action of `piece_id` when it is a guard.
    fn pop_guard(&mut self, piece_id: PieceId) -> Option<QueuedAction> {
        let queue = self.queues.get_mut(&piece_id)?;
        let guard =
            queue.pop_back_if(|queued| matches!(queued.action, PieceAction::Guard { .. }))?;
        if queue.is_empty() {
            self.queues.remove(&piece_id);
        }
        Some(guard)
    }

    /// Iterates over the next action of every piece that has actions waiting.
    fn fronts(&self) -> impl Iterator<Item = (PieceId, QueuedAction)> + '_ {
        self.queues
            .iter()
            .filter_map(|(piece_id, queue)| Some((*piece_id, *queue.front()?)))
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

    /// Forgets the failed actions of a player starting a new session.
    ///
    /// Action ids are only unique per connection, so a returning player may reuse old ids.
    pub(super) fn forget_player(&mut self, player_id: PlayerId) {
        self.cancelled.forget_player(player_id);
    }
}

impl World {
    /// Accepts an action request from `player_id`, running it now when nothing blocks it.
    ///
    /// A guard the piece was left with is superseded by the new action: it never fires, and
    /// is reported as resolved.
    pub(super) fn submit_action(
        &mut self,
        player_id: PlayerId,
        request: ActionRequest,
        now: TimestampMs,
    ) {
        let queued = self.actions.arrive(player_id, &request);
        let predecessor_failed = request
            .after
            .is_some_and(|after| self.actions.cancelled.contains(player_id, after));
        if predecessor_failed {
            return self.finish_action(queued, Err(GameError::ActionCancelled));
        }
        if let Err(error) = self.check_ownership(player_id, request.piece_id) {
            return self.finish_action(queued, Err(error));
        }

        if let Some(guard) = self.actions.pop_guard(request.piece_id) {
            self.finish_action(guard, Ok(()));
        }
        if let Err(rejected) = self.actions.push(request.piece_id, queued) {
            let error = GameError::Internal("Too many queued actions".to_string());
            self.finish_action(rejected, Err(error));
        }
        self.process_actions(now);
    }

    /// Drops every queued action `player_id` has on `piece_id`.
    pub(super) fn clear_actions(&mut self, player_id: PlayerId, piece_id: PieceId) {
        if self.check_ownership(player_id, piece_id).is_ok() {
            self.cancel_queue(piece_id);
        }
    }

    /// Runs queued actions until none is ready, always the earliest requested ready one next.
    ///
    /// Readiness is checked again after every action, so a guard whose piece was just
    /// captured takes the square back within the same pass.
    pub(super) fn process_actions(&mut self, now: TimestampMs) {
        while let Some((piece_id, next)) = self.next_ready_action(now) {
            self.actions.pop_front(piece_id);
            if !self.run_action(piece_id, next, now) {
                self.cancel_queue(piece_id);
            }
        }
    }

    /// Returns the earliest requested action that is next in its piece queue and ready.
    fn next_ready_action(&self, now: TimestampMs) -> Option<(PieceId, QueuedAction)> {
        self.actions
            .fronts()
            .filter(|(piece_id, queued)| self.is_action_ready(*piece_id, queued.action, now))
            .min_by_key(|(_, queued)| queued.arrival)
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
    /// Guards wait while a piece of their owner holds the guarded square, then for the
    /// countdown and cooldown once an enemy took it; a square left empty fails them at once.
    fn is_action_ready(&self, piece_id: PieceId, action: PieceAction, now: TimestampMs) -> bool {
        if !action.needs_cooldown() {
            return true;
        }
        let piece = self.state.pieces.get(&piece_id);
        if let (PieceAction::Guard { target }, Some(guard)) = (action, piece) {
            match self.state.piece_at(target) {
                None => return true,
                Some(occupant)
                    if occupant.id != guard.id && occupant.owner_id == guard.owner_id =>
                {
                    return false;
                }
                Some(_) => {}
            }
        }
        !self.clock.moves_locked(now) && piece.is_none_or(|piece| piece.is_ready(now))
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
        let culprits = match (
            &result,
            queued.action.move_target(),
            self.state.pieces.get(&piece_id),
        ) {
            (Err(_), Some(target), Some(piece)) => {
                self.setup
                    .catalog
                    .move_culprits(&self.state, piece, target, MoveTiming::Now)
            }
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
        self.apply_effects(Some(queued.player_id), effects, now);
        Ok(())
    }
}
