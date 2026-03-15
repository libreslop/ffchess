use crate::reducer::types::GameStateReducer;
use common::*;

/// Snapshot payload from the server for a state update.
pub struct UpdateStateParams {
    pub players: Vec<Player>,
    pub pieces: Vec<Piece>,
    pub shops: Vec<Shop>,
    pub removed_pieces: Vec<PieceId>,
    pub removed_players: Vec<PlayerId>,
    pub board_size: i32,
}

pub fn handle_update_state(
    next: &mut GameStateReducer,
    params: UpdateStateParams,
) {
    next.error = None;
    next.disconnected = false;
    next.state.board_size = params.board_size;
    let player_id_val = next.player_id.unwrap_or_else(PlayerId::nil);

    #[cfg(target_arch = "wasm32")]
    let now_ms = js_sys::Date::now() as i64;
    #[cfg(not(target_arch = "wasm32"))]
    let now_ms = chrono::Utc::now().timestamp_millis();

    for p in params.players {
        if next.player_id == Some(p.id) {
            next.last_score = p.score;
            next.last_kills = p.kills;
            next.last_captured = p.pieces_captured;
            next.last_survival_secs = ((now_ms - p.join_time).max(0) / 1000) as u64;
        }
        next.state.players.insert(p.id, p);
    }

    for mut p in params.pieces {
        if p.owner_id == Some(player_id_val)
            && let Some(old_p) = next.state.pieces.get(&p.id)
        {
            p.last_move_time = old_p.last_move_time;
            p.cooldown_ms = old_p.cooldown_ms;
        }

        if let Some(pm) = next
            .pm_queue
            .iter()
            .find(|pm| pm.piece_id == p.id && pm.pending)
            && p.position != pm.target
            && let Some(old_p) = next.state.pieces.get(&p.id)
        {
            p.position = old_p.position;
        }

        if let Some(match_idx) = next
            .pm_queue
            .iter()
            .rposition(|pm| pm.piece_id == p.id && pm.target == p.position)
        {
            let mut i = 0;
            let mut threshold = match_idx;
            while i <= threshold {
                if next.pm_queue[i].piece_id == p.id {
                    next.pm_queue.remove(i);
                    if threshold == 0 {
                        break;
                    }
                    threshold -= 1;
                } else {
                    i += 1;
                }
            }
        }
        next.state.pieces.insert(p.id, p);
    }

    next.state.shops = params.shops;
    for id in params.removed_pieces {
        next.state.pieces.remove(&id);
        next.pm_queue.retain(|pm| pm.piece_id != id);
    }
    for id in params.removed_players {
        next.state.players.remove(&id);
    }

    if let Some(player_id) = next.player_id {
        if player_id != PlayerId::nil() {
            next.is_dead = !next.state.players.contains_key(&player_id);
        }
    }
}
