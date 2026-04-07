//! Reducer helper functions for applying server updates.

use super::actions::UpdateStatePayload;
use super::time::now_timestamp_ms;
use crate::reducer::types::GameStateReducer;
use common::types::PlayerId;

impl GameStateReducer {
    /// Applies a server update payload to this reducer state.
    pub fn apply_update_state(&mut self, params: UpdateStatePayload) {
        self.error = None;
        self.disconnected = false;
        self.state.board_size = params.board_size;
        let local_player_id = self.player_id.unwrap_or_else(PlayerId::nil);
        if local_player_id == PlayerId::nil() {
            if let Some(preview_state) = self.menu_preview_state.clone() {
                self.state = preview_state;
            }
            self.pm_queue.clear();
            self.is_dead = false;
            return;
        }

        let now_ms = now_timestamp_ms();

        for player in params.players {
            if self.player_id == Some(player.id) && !self.is_dead && !self.is_victory {
                self.last_score = player.score;
                self.last_kills = player.kills;
                self.last_captured = player.pieces_captured;
                self.last_survival_secs = (now_ms - player.join_time).as_u64() / 1000;
            }
            self.state.players.insert(player.id, player);
        }

        for mut piece in params.pieces {
            if piece.owner_id == Some(local_player_id)
                && let Some(previous_piece) = self.state.pieces.get(&piece.id)
            {
                piece.last_move_time = previous_piece.last_move_time;
                piece.cooldown_ms = previous_piece.cooldown_ms;
            }

            if let Some(pending_move) = self
                .pm_queue
                .iter()
                .find(|pending_move| pending_move.piece_id == piece.id && pending_move.pending)
                && piece.position != pending_move.target
                && let Some(previous_piece) = self.state.pieces.get(&piece.id)
            {
                piece.position = previous_piece.position;
            }

            if let Some(matching_pending_index) = self.pm_queue.iter().rposition(|pending_move| {
                pending_move.piece_id == piece.id && pending_move.target == piece.position
            }) {
                let mut index = 0;
                let mut upper_bound = matching_pending_index;
                while index <= upper_bound {
                    if self.pm_queue[index].piece_id == piece.id {
                        self.pm_queue.remove(index);
                        if upper_bound == 0 {
                            break;
                        }
                        upper_bound -= 1;
                    } else {
                        index += 1;
                    }
                }
            }
            self.state.pieces.insert(piece.id, piece);
        }

        self.state.shops = params.shops;
        for piece_id in params.removed_pieces {
            self.state.pieces.remove(&piece_id);
            self.pm_queue
                .retain(|pending_move| pending_move.piece_id != piece_id);
        }
        for player_id in params.removed_players {
            self.state.players.remove(&player_id);
        }

        if self.queue_status.is_some() {
            self.is_dead = false;
        } else if let Some(player_id) = self.player_id
            && player_id != PlayerId::nil()
        {
            self.is_dead = !self.state.players.contains_key(&player_id);
        }
    }
}
