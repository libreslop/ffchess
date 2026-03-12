use crate::reducer::types::GameStateReducer;
use common::*;
use uuid::Uuid;

pub fn handle_update_state(
    next: &mut GameStateReducer,
    players: Vec<Player>,
    pieces: Vec<Piece>,
    shops: Vec<Shop>,
    removed_pieces: Vec<Uuid>,
    removed_players: Vec<Uuid>,
    board_size: i32,
) {
    next.error = None;
    next.disconnected = false;
    next.state.board_size = board_size;
    let player_id_val = next.player_id.unwrap_or_else(Uuid::nil);

    #[cfg(target_arch = "wasm32")]
    let now_secs = (js_sys::Date::now() / 1000.0) as i64;
    #[cfg(not(target_arch = "wasm32"))]
    let now_secs = chrono::Utc::now().timestamp();

    for p in players {
        if next.player_id == Some(p.id) {
            next.last_score = p.score;
            next.last_kills = p.kills;
            next.last_captured = p.pieces_captured;
            next.last_survival_secs = (now_secs - p.join_time).max(0) as u64;
        }
        next.state.players.insert(p.id, p);
    }

    for mut p in pieces {
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

    next.state.shops = shops;
    for id in removed_pieces {
        next.state.pieces.remove(&id);
        next.pm_queue.retain(|pm| pm.piece_id != id);
    }
    for id in removed_players {
        next.state.players.remove(&id);
    }
}
