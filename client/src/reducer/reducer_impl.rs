//! Reducer implementation for applying actions to client state.

use super::actions::{GameAction, InitPayload};
use super::handlers::handle_update_state;
use super::types::{ClientPhase, GameStateReducer};
use common::logic::calculate_cooldown;
use common::protocol::{ClientMessage, GameError};
use common::types::{DurationMs, PieceId, PlayerId, TimestampMs};
use std::rc::Rc;
use yew::prelude::*;

impl Reducible for GameStateReducer {
    type Action = GameAction;

    /// Applies a reducer action and returns the next state.
    ///
    /// `action` is the incoming `GameAction`. Returns a new `GameStateReducer` in an `Rc`.
    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut next = (*self).clone();
        match action {
            GameAction::SetInit(payload) => {
                let InitPayload {
                    player_id,
                    session_secret,
                    state,
                    mode,
                    pieces,
                    shops,
                } = *payload;
                next.player_id = Some(player_id);
                next.session_secret = Some(session_secret);
                next.state = state;
                next.mode = Some(mode);
                next.piece_configs = pieces;
                next.shop_configs = shops;
                next.pm_queue.clear();
                next.error = None;
                next.queue_status = None;
                next.disconnected = false;
                next.is_victory = false;
                next.victory_title = None;
                next.victory_msg = None;
                next.disconnected_title = None;
                next.disconnected_msg = None;

                if player_id != PlayerId::nil() {
                    next.fatal_error = false;
                    if let Some(p) = next.state.players.get(&player_id) {
                        next.last_score = p.score;
                        next.is_dead = false;
                    } else {
                        // If player is not in state yet, don't immediately set is_dead to true
                        // as it might be a race condition during Init.
                        // The next UpdateState will correct it if they are truly gone.
                        next.is_dead = false;
                    }
                } else {
                    // Queue Init snapshots use nil player id. Never carry over dead state.
                    next.is_dead = false;
                }
            }
            GameAction::SetQueueStatus(status) => {
                next.queue_status = Some(status);
                next.error = None;
                next.disconnected = false;
                next.is_dead = false;
                next.is_victory = false;
                next.victory_title = None;
                next.victory_msg = None;
            }
            GameAction::UpdateState(payload) => {
                handle_update_state(&mut next, *payload);
            }
            GameAction::SetError(e) => {
                next.error = (!matches!(e, GameError::TargetFriendly)).then_some(e.clone());
                // Identify the first pending move; errors from the server always correspond to a pending move
                let failing_piece_id = next
                    .pm_queue
                    .iter()
                    .find(|pm| pm.pending)
                    .map(|pm| pm.piece_id);

                match e {
                    GameError::OnCooldown => {
                        if let Some(pid) = failing_piece_id {
                            for pm in next.pm_queue.iter_mut() {
                                if pm.piece_id == pid {
                                    pm.pending = false;
                                }
                            }
                        } else {
                            // Fallback: reset all
                            for pm in next.pm_queue.iter_mut() {
                                pm.pending = false;
                            }
                        }
                    }
                    _ => {
                        if let Some(pid) = failing_piece_id {
                            // Revert state for the failing piece only
                            if let Some(pm) = next
                                .pm_queue
                                .iter()
                                .find(|pm| pm.piece_id == pid && pm.pending)
                                && let Some(p) = next.state.pieces.get_mut(&pid)
                            {
                                p.last_move_time = pm.old_last_move_time;
                                p.cooldown_ms = pm.old_cooldown_ms;
                            }
                            next.pm_queue.retain(|pm| pm.piece_id != pid);
                        } else {
                            // Unknown source; fall back to previous behaviour
                            for pm in next.pm_queue.iter().rev() {
                                if pm.pending
                                    && let Some(p) = next.state.pieces.get_mut(&pm.piece_id)
                                {
                                    p.last_move_time = pm.old_last_move_time;
                                    p.cooldown_ms = pm.old_cooldown_ms;
                                }
                            }
                            next.pm_queue.clear();
                        }
                    }
                }
            }
            GameAction::SetVictory { title, msg } => {
                next.error = None;
                next.disconnected = false;
                next.fatal_error = false;
                next.is_dead = false;
                if let Some(player_id) = next.player_id
                    && let Some(player) = next.state.players.get(&player_id)
                {
                    next.last_score = player.score;
                    next.last_kills = player.kills;
                    next.last_captured = player.pieces_captured;
                    let now = current_timestamp_ms();
                    next.last_survival_secs = (now - player.join_time).as_u64() / 1000;
                }
                next.is_victory = true;
                next.disconnected_title = None;
                next.disconnected_msg = None;
                next.victory_title = Some(title);
                next.victory_msg = Some(msg);
            }
            GameAction::GameOver {
                final_score,
                kills,
                pieces_captured,
                time_survived_secs,
            } => {
                next.last_score = final_score;
                next.last_kills = kills;
                next.last_captured = pieces_captured;
                next.last_survival_secs = time_survived_secs;
                next.is_victory = false;
                next.victory_title = None;
                next.victory_msg = None;
                next.is_dead = true;
            }
            GameAction::AddPmove(pm) => {
                next.pm_queue.push(pm);
            }
            GameAction::ClearPmQueue(piece_id) => {
                if piece_id == PieceId::nil() {
                    next.pm_queue.clear();
                } else {
                    next.pm_queue.retain(|pm| pm.piece_id != piece_id);
                }
            }
            GameAction::Tick(tx) => {
                #[cfg(target_arch = "wasm32")]
                let now = TimestampMs::from_millis(js_sys::Date::now() as i64);
                #[cfg(not(target_arch = "wasm32"))]
                let now = TimestampMs::from_millis(chrono::Utc::now().timestamp_millis());

                let mut processed_pieces = std::collections::HashSet::<PieceId>::new();
                let mut blocked_pieces = std::collections::HashSet::<PieceId>::new();
                let player_id = next.player_id.unwrap_or_else(PlayerId::nil);
                for pm in next.pm_queue.iter_mut() {
                    if processed_pieces.contains(&pm.piece_id) || pm.pending {
                        processed_pieces.insert(pm.piece_id);
                        continue;
                    }
                    if let Some(piece) = next.state.pieces.get(&pm.piece_id)
                        && now
                            >= piece.last_move_time
                                + piece.cooldown_ms
                                + DurationMs::from_millis(50)
                    {
                        let target_has_friendly_piece = next.state.pieces.values().any(|other| {
                            other.position == pm.target
                                && other.owner_id == Some(player_id)
                                && other.id != pm.piece_id
                        });
                        if target_has_friendly_piece {
                            log_client_error(
                                "Skipping move because the target square is occupied by a friendly piece.",
                            );
                            blocked_pieces.insert(pm.piece_id);
                            processed_pieces.insert(pm.piece_id);
                            continue;
                        }

                        let _ = tx.0.send(ClientMessage::MovePiece {
                            piece_id: pm.piece_id,
                            target: pm.target,
                        });
                        pm.pending = true;
                        processed_pieces.insert(pm.piece_id);

                        if let Some(p) = next.state.pieces.get_mut(&pm.piece_id) {
                            pm.old_last_move_time = p.last_move_time;
                            pm.old_cooldown_ms = p.cooldown_ms;

                            if let Some(config) = next.piece_configs.get(&p.piece_type) {
                                p.cooldown_ms = calculate_cooldown(config);
                            }
                            p.last_move_time = now;
                        }
                    }
                }
                next.pm_queue
                    .retain(|pm| !blocked_pieces.contains(&pm.piece_id));
            }
            GameAction::Pong(t) => {
                let now = js_sys::Date::now() as u64;
                if now >= t {
                    next.ping_ms = now - t;
                }
            }
            GameAction::SetFPS(fps) => {
                next.fps = fps;
            }
            GameAction::SetDisconnected {
                disconnected,
                is_fatal,
                title,
                msg,
            } => {
                next.disconnected = disconnected;
                next.fatal_error = is_fatal;
                next.disconnected_title = title;
                next.disconnected_msg = msg;
                if disconnected || is_fatal {
                    next.is_victory = false;
                    next.victory_title = None;
                    next.victory_msg = None;
                }
            }
            GameAction::Reset => {
                next.player_id = Some(PlayerId::nil());
                next.session_secret = None;
                next.state = common::models::GameState::default();
                next.pm_queue.clear();
                next.error = None;
                next.disconnected = false;
                next.fatal_error = false;
                next.is_dead = false;
                next.is_victory = false;
                next.queue_status = None;
                next.victory_title = None;
                next.victory_msg = None;
                next.disconnected_title = None;
                next.disconnected_msg = None;
                // Keep mode and configs so the kit list can render while reconnecting
            }
        }
        next.phase = compute_phase(&next);
        next.into()
    }
}

/// Logs a client-side error without surfacing it in the game UI.
fn log_client_error(message: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&message.into());
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{message}");
}

fn current_timestamp_ms() -> TimestampMs {
    #[cfg(target_arch = "wasm32")]
    {
        TimestampMs::from_millis(js_sys::Date::now() as i64)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        TimestampMs::from_millis(chrono::Utc::now().timestamp_millis())
    }
}

fn compute_phase(state: &GameStateReducer) -> ClientPhase {
    // Phase is derived from authoritative server state plus the local death flag.
    let Some(player_id) = state.player_id else {
        return ClientPhase::Menu;
    };
    if player_id == PlayerId::nil() {
        if state.queue_status.is_some() {
            return ClientPhase::Queued;
        }
        return ClientPhase::Menu;
    }
    if state.is_dead || state.is_victory {
        return ClientPhase::Dead;
    }
    if let Some(player) = state.state.players.get(&player_id)
        && state.state.pieces.contains_key(&player.king_id)
    {
        return ClientPhase::Alive;
    }
    ClientPhase::Joining
}
