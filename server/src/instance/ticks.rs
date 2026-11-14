//! Periodic simulation step of a game instance.

use super::world::World;
use crate::outbound::EncodedUpdate;
use common::types::{DurationMs, TimestampMs};

/// Instances nobody has looked at for this long stop simulating.
const IDLE_AFTER: DurationMs = DurationMs(5000);

/// Housekeeping runs once every this many ticks (about a minute at 10 ticks per second).
const HOUSEKEEPING_EVERY_TICKS: u64 = 600;

impl World {
    /// Runs one tick: actions, NPCs, board resizing, hooks, then a snapshot broadcast.
    pub(super) fn tick(&mut self, now: TimestampMs) {
        if !self.outbox.is_empty() {
            self.clock.last_viewed_at = now;
        }
        if now - self.clock.last_viewed_at < IDLE_AFTER {
            self.process_actions(now);
            self.spawn_npcs(now);
            self.tick_npcs(now);
        }

        if self.clock.ticks.is_multiple_of(HOUSEKEEPING_EVERY_TICKS) {
            self.sessions.prune(now);
            self.colors.cleanup(now);
        }
        self.clock.ticks += 1;
        for player_id in self.outbox.player_ids() {
            self.colors.update_activity(player_id, now);
        }

        self.shrink_board();
        self.resolve_hooks(now);
        self.release_spectators_if_finished();
        // Encoded once here, so every connection writes the same text without re-encoding it.
        if let Some(update) = EncodedUpdate::encode(&self.update_message()) {
            self.outbox.publish(&update);
        }
    }

    /// Shrinks the board to the mode's size for the current player count.
    ///
    /// The board only shrinks once no player piece would end up outside it.
    fn shrink_board(&mut self) {
        let target = self.setup.mode.board_size_for(self.state.players.len());
        if target >= self.state.board_size {
            return;
        }
        let strands_player_piece = self
            .state
            .pieces
            .values()
            .any(|piece| piece.is_player_owned() && !target.contains(piece.position));
        if !strands_player_piece {
            self.state.board_size = target;
            let removed = self.state.prune_out_of_bounds();
            self.cancel_actions_of(removed);
        }
    }
}
