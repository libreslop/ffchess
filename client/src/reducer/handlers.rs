//! Reducer helper functions for applying server updates.

use super::actions::UpdateStatePayload;
use super::time::now_timestamp_ms;
use crate::reducer::types::GameStateReducer;
use std::collections::{HashMap, HashSet};

impl GameStateReducer {
    /// Applies a server update payload to this reducer state.
    pub fn apply_update_state(&mut self, params: UpdateStatePayload) {
        self.error = None;
        self.disconnected = false;
        self.state.board_size = params.board_size;
        let local_player_id = self.active_player_id();
        if local_player_id.is_none() {
            self.pm_queue.clear();
            self.is_dead = false;
        }

        let now_ms = now_timestamp_ms();
        let incoming_piece_ids: HashSet<_> = params.pieces.iter().map(|piece| piece.id).collect();
        let mut reassigned_piece_ids = HashMap::new();

        for incoming_piece in &params.pieces {
            if self.state.pieces.contains_key(&incoming_piece.id) {
                continue;
            }
            let mut candidates = self
                .state
                .pieces
                .values()
                .filter(|existing_piece| {
                    existing_piece.owner_id == incoming_piece.owner_id
                        && existing_piece.position == incoming_piece.position
                        && !incoming_piece_ids.contains(&existing_piece.id)
                })
                .map(|piece| piece.id);

            let first_candidate = candidates.next();
            let has_multiple = candidates.next().is_some();
            if !has_multiple
                && let Some(old_id) = first_candidate
            {
                reassigned_piece_ids.insert(old_id, incoming_piece.id);
            }
        }

        if !reassigned_piece_ids.is_empty() {
            for pending_move in &mut self.pm_queue {
                if let Some(new_id) = reassigned_piece_ids.get(&pending_move.piece_id) {
                    web_sys::console::log_1(
                        &format!(
                            "Remapping queued move piece id {} -> {} after server update.",
                            pending_move.piece_id, new_id
                        )
                        .into(),
                    );
                    pending_move.piece_id = *new_id;
                }
            }
        }

        for player in params.players {
            if let Some(_) = local_player_id
                && self.player_id == Some(player.id)
                && !self.is_dead
            {
                self.last_score = player.score;
                self.last_kills = player.kills;
                self.last_captured = player.pieces_captured;

                if !self.is_victory {
                    self.last_survival_secs = (now_ms - player.join_time).as_u64() / 1000;
                }
            }
            self.state.players.insert(player.id, player);
        }

        for piece in params.pieces {
            if let Some(matching_pending_index) = self.pm_queue.iter().rposition(|pending_move| {
                pending_move.shop_item_index.is_none()
                    && pending_move.piece_id == piece.id
                    && pending_move.target == piece.position
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
        } else if let Some(player_id) = self.active_player_id() {
            self.is_dead = !self.state.players.contains_key(&player_id);
        } else {
            self.is_dead = false;
        }

        if local_player_id.is_none() {
            self.menu_preview_state = Some(self.state.clone());
        }
    }
}
