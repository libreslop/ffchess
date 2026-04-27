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
                    state,
                    mode,
                    pieces,
                    shops,
                    sync_interval_ms,
                } = *payload;
                next.player_id = Some(player_id);
                next.session_secret = Some(session_secret);
                next.state = state;
                next.mode = Some(mode);
                next.piece_configs = pieces;
                next.shop_configs = shops;
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
            GameAction::SetError(e) => {
                next.error = (!matches!(e, GameError::TargetFriendly)).then_some(e.clone());
                if is_move_error(&e) && !next.pm_queue.is_empty() {
                    if let Some(index) = next
                        .pm_queue
                        .iter()
                        .position(|pm| pm.shop_item_index.is_none())
                    {
                        next.pm_queue.remove(index);
                    }
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
            GameAction::Reset => {
                next.player_id = Some(PlayerId::nil());
                next.session_secret = None;
                next.state = next.menu_preview_state.clone().unwrap_or_default();
                next.pm_queue.clear();
                next.error = None;
                next.clear_disconnect_ui();
                next.fatal_error = false;
                next.is_dead = false;
                next.clear_victory_state();
                next.queue_status = None;
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
