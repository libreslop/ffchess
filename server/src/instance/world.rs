//! Mutable state of one game instance, guarded by a single lock.

use super::actions::ActionQueues;
use super::chat_history::ChatHistory;
use super::hooks::HookEvents;
use super::outbox::Outbox;
use super::sessions::SessionBook;
use super::setup::InstanceSetup;
use crate::colors::ColorManager;
use common::models::GameState;
use common::protocol::{ChatLine, ServerMessage};
use common::rules::ActionEffects;
use common::types::{DurationMs, PlayerId, SessionSecret, TimestampMs};
use std::collections::HashSet;
use std::sync::Arc;

/// Timestamps that drive instance activity and match start.
pub(super) struct InstanceClock {
    /// Last time anyone was attached to the instance.
    pub(super) last_viewed_at: TimestampMs,
    /// Last time the instance went from empty to having players.
    pub(super) last_started_at: TimestampMs,
    /// Moves are rejected until this time during a match countdown.
    pub(super) move_unlock_at: Option<TimestampMs>,
    /// Number of ticks processed so far.
    pub(super) ticks: u64,
}

impl InstanceClock {
    /// Creates a clock for an instance created at `now`.
    fn new(now: TimestampMs) -> Self {
        Self {
            last_viewed_at: now,
            last_started_at: now,
            move_unlock_at: None,
            ticks: 0,
        }
    }

    /// Returns true while the match countdown still blocks moves.
    pub(super) fn moves_locked(&self, now: TimestampMs) -> bool {
        self.move_unlock_at.is_some_and(|unlock_at| now < unlock_at)
    }
}

/// Everything that changes while an instance runs.
pub(super) struct World {
    pub(super) setup: Arc<InstanceSetup>,
    pub(super) state: GameState,
    pub(super) outbox: Outbox,
    pub(super) sessions: SessionBook,
    pub(super) actions: ActionQueues,
    pub(super) hooks: HookEvents,
    pub(super) chat: ChatHistory,
    pub(super) colors: ColorManager,
    /// Players that already won and only remain to view the final board.
    pub(super) victors: HashSet<PlayerId>,
    pub(super) clock: InstanceClock,
}

impl World {
    /// Creates an empty world sized for zero players.
    pub(super) fn new(setup: Arc<InstanceSetup>, chat_ttl: DurationMs, now: TimestampMs) -> Self {
        let state = GameState {
            board_size: setup.mode.board_size_for(0),
            mode_id: setup.mode.id.clone(),
            ..GameState::default()
        };
        Self {
            setup,
            state,
            outbox: Outbox::default(),
            sessions: SessionBook::default(),
            actions: ActionQueues::default(),
            hooks: HookEvents::default(),
            chat: ChatHistory::new(chat_ttl),
            colors: ColorManager::new(),
            victors: HashSet::new(),
            clock: InstanceClock::new(now),
        }
    }

    /// Returns the number of players still competing.
    pub(super) fn active_player_count(&self) -> usize {
        self.state
            .players
            .keys()
            .filter(|id| !self.victors.contains(id))
            .count()
    }

    /// Handles the consequences of an applied action: captures, disturbed partner pieces, and
    /// depleted shops.
    ///
    /// Queued actions of partner pieces were planned from where they stood before, so they are
    /// cancelled.
    pub(super) fn apply_effects(&mut self, capturer: Option<PlayerId>, effects: ActionEffects) {
        self.cancel_actions_of(effects.partners);
        for captured in &effects.captured {
            self.record_capture(capturer, captured);
        }
        for shop_id in effects.depleted_shops {
            self.respawn_shop(&shop_id);
        }
    }

    /// Appends a chat line to the room history and sends it to everyone.
    pub(super) fn post_chat(&mut self, line: ChatLine, now: TimestampMs) {
        self.chat.push(line.clone(), now);
        self.outbox.broadcast(&ServerMessage::Chat { line });
    }

    /// Builds the full snapshot sent to a client when it starts watching or playing.
    pub(super) fn init_message(
        &mut self,
        player_id: PlayerId,
        session_secret: SessionSecret,
        sync_interval_ms: u32,
        now: TimestampMs,
    ) -> ServerMessage {
        let mut state = self.state.clone();
        state.mode_id = self.setup.public_mode_id.clone();
        ServerMessage::Init {
            player_id,
            session_secret,
            move_unlock_at: self.clock.move_unlock_at,
            state: Box::new(state),
            mode: Box::new(self.setup.client_mode.clone()),
            catalog: Box::new((*self.setup.catalog).clone()),
            chat_room_key: self.setup.chat_room_key(),
            chat_history: self.chat.snapshot(now),
            sync_interval_ms,
        }
    }

    /// Builds the per-tick world snapshot.
    pub(super) fn update_message(&self) -> ServerMessage {
        ServerMessage::UpdateState {
            players: self.state.players.values().cloned().collect(),
            pieces: self.state.pieces.values().cloned().collect(),
            shops: self.state.shops.clone(),
            board_size: self.state.board_size,
        }
    }
}
