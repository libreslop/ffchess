//! Reducer helpers applying server snapshots.

use super::actions::{InitPayload, UpdateStatePayload};
use super::types::{ChatLog, GameStateReducer, MatchStats};
use std::rc::Rc;

impl GameStateReducer {
    /// Replaces the whole client view with an Init snapshot.
    pub(super) fn apply_init(&mut self, payload: InitPayload) {
        let InitPayload {
            player_id,
            session_secret,
            move_unlock_at,
            state,
            mode,
            catalog,
            chat_room_key,
            chat_history,
            spectators,
        } = payload;
        self.player_id = Some(player_id);
        self.session_secret = Some(session_secret);
        self.move_unlock_at = move_unlock_at;
        self.state = state;
        self.mode = Some(mode);
        self.catalog = Rc::new(catalog);
        self.spectators = spectators;
        self.chat = ChatLog {
            room_key: Some(chat_room_key),
            lines: chat_history,
        };
        self.premoves.clear();
        self.error = None;
        self.clear_disconnect_ui();
        self.clear_victory_state();
        // A missing player right after Init is a race with the first update, not a death.
        self.is_dead = false;

        if player_id.is_nil() {
            self.menu_preview_state = Some(self.state.clone());
        } else {
            self.queue_status = None;
            self.fatal_error = false;
            if let Some(player) = self.state.players.get(&player_id) {
                self.stats.score = player.score;
                self.stats.board_rotation_deg = player.board_rotation_deg;
            }
        }
    }

    /// Replaces the world with a per-tick snapshot and reconciles local predictions.
    pub(super) fn apply_update_state(&mut self, payload: UpdateStatePayload) {
        self.disconnected = false;
        self.state.board_size = payload.board_size;
        self.state.players = payload.players.into_iter().map(|p| (p.id, p)).collect();
        self.state.pieces = payload.pieces.into_iter().map(|p| (p.id, p)).collect();
        self.state.shops = payload.shops;
        self.spectators = payload.spectators;

        let Some(player_id) = self.active_player_id() else {
            self.premoves.clear();
            self.is_dead = false;
            self.menu_preview_state = Some(self.state.clone());
            return;
        };

        self.premoves.sync_with(&self.state);
        if !self.is_dead
            && let Some(player) = self.state.players.get(&player_id)
        {
            let survival_secs = self.stats.survival_secs;
            self.stats = MatchStats::of(player, self.server_now());
            if self.is_victory() {
                // The survival time freezes at the moment of victory.
                self.stats.survival_secs = survival_secs;
            }
        }
        self.is_dead = self.queue_status.is_none() && !self.state.players.contains_key(&player_id);
    }

    /// Snapshots the current local player's live stats into end-screen fields.
    pub(super) fn snapshot_stats(&mut self) {
        if let Some(player) = self
            .player_id
            .and_then(|player_id| self.state.players.get(&player_id))
        {
            self.stats = MatchStats::of(player, self.server_now());
        }
    }
}
