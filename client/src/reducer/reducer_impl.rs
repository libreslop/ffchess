//! Reducer implementation for applying actions to client state.

use super::actions::{GameAction, InitPayload};
use super::time::now_timestamp_ms;
use super::types::{ClientPhase, GameStateReducer};
use common::protocol::{GameError, VictoryFocusTarget};
use common::types::PlayerId;
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
                    move_unlock_at,
                    state,
                    mode,
                    pieces,
                    shops,
                    chat_room_key,
                    chat_history,
                    sync_interval_ms,
                } = *payload;
                next.player_id = Some(player_id);
                next.session_secret = Some(session_secret);
                next.move_unlock_at = move_unlock_at;
                next.state = state;
                next.mode = Some(mode);
                next.piece_configs = pieces;
                next.shop_configs = shops;
                next.chat_room_key = Some(chat_room_key);
                next.chat_lines = chat_history;
                next.sync_interval_ms = sync_interval_ms;
                next.pm_queue.clear();
                next.error = None;
                if player_id != PlayerId::nil() {
                    next.queue_status = None;
                }
                next.clear_disconnect_ui();
                next.clear_victory_state();

                if player_id == PlayerId::nil() {
                    next.menu_preview_state = Some(next.state.clone());
                }

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
                next.clear_disconnect_ui();
                next.is_dead = false;
                next.clear_victory_state();
            }
            GameAction::UpdateState(payload) => {
                next.apply_update_state(*payload);
            }
            GameAction::PushChatLine(line) => {
                const MAX_CHAT_LINES: usize = 120;
                next.chat_lines.push(line);
                if next.chat_lines.len() > MAX_CHAT_LINES {
                    let drop_count = next.chat_lines.len() - MAX_CHAT_LINES;
                    next.chat_lines.drain(0..drop_count);
                }
            }
            GameAction::PruneExpiredChat { now, ttl_ms } => {
                prune_expired_chat_lines(&mut next.chat_lines, now, ttl_ms);
            }
            GameAction::SetError(e) => {
                next.error = (!matches!(e, GameError::TargetFriendly)).then_some(e.clone());
                if matches!(e, GameError::PieceNotFound) {
                    let before = next.pm_queue.len();
                    next.pm_queue
                        .retain(|pm| next.state.pieces.contains_key(&pm.piece_id));
                    if before != next.pm_queue.len() {
                        web_sys::console::error_1(
                            &format!(
                                "Dropped {} stale premoves after PieceNotFound.",
                                before - next.pm_queue.len()
                            )
                            .into(),
                        );
                    }
                } else if is_move_error(&e)
                    && !next.pm_queue.is_empty()
                    && let Some(index) = next
                        .pm_queue
                        .iter()
                        .position(|pm| pm.shop_item_index.is_none())
                {
                    next.pm_queue.remove(index);
                }
            }
            GameAction::SetVictory {
                title,
                msg,
                focus_target,
            } => {
                next.error = None;
                next.clear_disconnect_ui();
                next.fatal_error = false;
                next.is_dead = false;
                next.snapshot_last_stats_from_current_player();
                next.is_victory = true;
                next.victory_title = Some(title);
                next.victory_msg = Some(msg);
                next.victory_focus_target = focus_target;
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
                next.clear_victory_messages();
                next.is_dead = true;
            }
            GameAction::AddPmove(pm) => {
                next.pm_queue.push(pm);
            }
            GameAction::RemovePm(pm_id) => {
                next.pm_queue.retain(|pm| pm.id != pm_id);
            }
            GameAction::ClearPm(piece_id) => {
                next.pm_queue.retain(|pm| pm.piece_id != piece_id);
            }
            GameAction::Pong(t, server_time) => {
                let now = js_sys::Date::now() as u64;
                if now >= t {
                    next.ping_ms = now - t;
                    let latency = (now - t) / 2;
                    let sample_offset = server_time.as_i64() - (t + latency) as i64;

                    if next.clock_offset_ms == 0 {
                        next.clock_offset_ms = sample_offset;
                    } else {
                        // EMA with alpha = 0.2
                        next.clock_offset_ms =
                            (next.clock_offset_ms as f64 * 0.8 + sample_offset as f64 * 0.2) as i64;
                    }
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
                    next.clear_victory_state();
                }
            }
            GameAction::ResetForRejoin => {
                next.player_id = Some(PlayerId::nil());
                next.session_secret = None;
                next.move_unlock_at = None;
                next.state = next.menu_preview_state.clone().unwrap_or_default();
                next.pm_queue.clear();
                next.error = None;
                next.clear_disconnect_ui();
                next.fatal_error = false;
                next.is_dead = false;
                next.clear_victory_state();
                next.queue_status = None;
                // Preserve chat context/lines while rejoin UI is shown so room chat
                // remains visible until the next Init snapshot confirms a room switch.
            }
            GameAction::Reset => {
                next.player_id = Some(PlayerId::nil());
                next.session_secret = None;
                next.move_unlock_at = None;
                next.state = next.menu_preview_state.clone().unwrap_or_default();
                next.pm_queue.clear();
                next.error = None;
                next.clear_disconnect_ui();
                next.fatal_error = false;
                next.is_dead = false;
                next.clear_victory_state();
                next.queue_status = None;
                next.chat_room_key = None;
                next.chat_lines.clear();
                // Keep state/mode/configs so the join overlay shows the same preview board
                // as a fresh menu screen without a visual blank between flows.
            }
        }
        next.phase = compute_phase(&next);
        next.into()
    }
}

/// Returns true when the server error likely corresponds to a rejected move request.
fn is_move_error(error: &GameError) -> bool {
    matches!(
        error,
        GameError::PieceNotFound
            | GameError::NotYourPiece
            | GameError::OnCooldown
            | GameError::TargetFriendly
            | GameError::InvalidMove
            | GameError::PathBlocked
    )
}

impl GameStateReducer {
    /// Clears reconnect/disconnect UI details while keeping fatal flags untouched.
    fn clear_disconnect_ui(&mut self) {
        self.disconnected = false;
        self.disconnected_title = None;
        self.disconnected_msg = None;
    }

    /// Clears only victory message text.
    fn clear_victory_messages(&mut self) {
        self.victory_title = None;
        self.victory_msg = None;
    }

    /// Clears all victory state so generic end overlays can be derived cleanly.
    fn clear_victory_state(&mut self) {
        self.is_victory = false;
        self.clear_victory_messages();
        self.victory_focus_target = VictoryFocusTarget::KeepCurrent;
    }

    /// Snapshots the current local player's live stats into end-screen fields.
    fn snapshot_last_stats_from_current_player(&mut self) {
        if let Some(player_id) = self.player_id
            && let Some(player) = self.state.players.get(&player_id)
        {
            self.last_score = player.score;
            self.last_kills = player.kills;
            self.last_captured = player.pieces_captured;
            let now = now_timestamp_ms();
            self.last_survival_secs = (now - player.join_time).as_u64() / 1000;
        }
    }
}

fn compute_phase(state: &GameStateReducer) -> ClientPhase {
    // Phase is derived from authoritative server state plus the local death flag.
    let Some(player_id) = state.active_player_id() else {
        if state.queue_status.is_some() {
            return ClientPhase::Queued;
        }
        return ClientPhase::Menu;
    };
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

fn prune_expired_chat_lines(
    lines: &mut Vec<common::protocol::ChatLine>,
    now: common::types::TimestampMs,
    ttl_ms: u32,
) {
    const CHAT_FADE_OUT_MS: i64 = 500;
    let prune_after_ms = ttl_ms.max(1) as i64 + CHAT_FADE_OUT_MS;
    lines.retain(|line| now.as_i64().saturating_sub(line.sent_at.as_i64()) < prune_after_ms);
}
