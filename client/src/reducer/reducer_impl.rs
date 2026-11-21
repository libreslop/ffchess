//! Reducer implementation for applying actions to client state.

use super::actions::GameAction;
use super::types::{
    ClientPhase, DisconnectNotice, ErrorNotice, FinishedMatch, GameStateReducer, MatchStats,
    SpectateStatus, Victory,
};
use common::protocol::{ChatLine, ClientMessage, GameError};
use common::types::{PlayerId, TimestampMs};
use std::rc::Rc;
use yew::prelude::*;

/// Maximum number of chat lines kept in memory.
const MAX_CHAT_LINES: usize = 120;

/// Extra time a chat line stays after its TTL so it can fade out.
const CHAT_FADE_OUT_MS: i64 = 500;

/// Weight of a new clock offset sample in the running average.
const CLOCK_OFFSET_SMOOTHING: f64 = 0.2;

impl Reducible for GameStateReducer {
    type Action = GameAction;

    /// Applies a reducer action and returns the next state.
    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut next = (*self).clone();
        match action {
            GameAction::SetInit(payload) => next.apply_init(*payload),
            GameAction::SetQueueStatus(status) => {
                next.queue_status = Some(status.continuing(next.queue_status.as_ref()));
                next.error = None;
                next.connection.clear_overlay();
                next.is_dead = false;
                next.clear_victory_state();
            }
            GameAction::UpdateState(payload) => next.apply_update_state(*payload),
            GameAction::QueueAction {
                piece_id,
                action,
                tx,
            } => {
                let request = next.premoves.enqueue(piece_id, action);
                if !tx.send(ClientMessage::QueueAction(request)) {
                    next.premoves
                        .resolve(request.id, Err(GameError::ActionCancelled), Vec::new());
                }
            }
            GameAction::ClearActions { piece_id, tx } => {
                next.premoves.clear_piece(piece_id);
                tx.send(ClientMessage::ClearActions { piece_id });
            }
            GameAction::ActionResult {
                id,
                result,
                culprits,
            } => {
                if let Some(error) = next.premoves.resolve(id, result, culprits) {
                    next.show_error(error);
                }
            }
            GameAction::PushChatLine(line) => {
                next.chat.lines.push(line);
                let excess = next.chat.lines.len().saturating_sub(MAX_CHAT_LINES);
                next.chat.lines.drain(..excess);
            }
            GameAction::PruneExpiredChat { now, ttl_ms } => {
                prune_expired_chat_lines(&mut next.chat.lines, now, ttl_ms);
            }
            GameAction::SetError(error) => next.show_error(error),
            GameAction::ClearError(serial) => {
                if next
                    .error
                    .as_ref()
                    .is_some_and(|notice| notice.serial == serial)
                {
                    next.error = None;
                }
            }
            GameAction::SetVictory { title, msg } => {
                next.error = None;
                next.connection = DisconnectNotice::none();
                next.is_dead = false;
                next.snapshot_stats();
                next.victory = Some(Victory {
                    title,
                    message: msg,
                });
            }
            GameAction::SetCamera(cue) => next.camera_cue = Some(Rc::new(cue)),
            // A session the client already left has no end screen to show.
            GameAction::GameOver(_) if next.active_player_id().is_none() => {}
            GameAction::GameOver(stats) => {
                next.stats = MatchStats::of_game_over(stats);
                next.victory = None;
                next.is_dead = true;
            }
            GameAction::Pong(sent_at, server_time) => next.record_pong(sent_at, server_time),
            GameAction::SetFPS(fps) => next.fps = fps,
            GameAction::SetDisconnected(notice) => {
                // A refused spectate request never gets an Init, so the request is dropped
                // and the menu comes back.
                if notice.is_fatal && next.spectate == Some(SpectateStatus::Watching) {
                    next.reset_session();
                }
                next.set_disconnect_notice(notice);
            }
            GameAction::ConnectionEstablished => {
                // The server drops queue entries and spectators along with their connection.
                next.queue_status = None;
                next.spectate = None;
            }
            GameAction::ResetForRejoin => {
                // Chat stays visible until the next Init confirms a room switch.
                next.reset_session();
            }
            GameAction::Reset => {
                // Mode and configs stay so the join overlay keeps showing the same board.
                next.reset_session();
                next.chat = Default::default();
            }
            GameAction::BeginSpectate => {
                // The board stays until the server's Init replaces it, so watching the match
                // just lost does not flash back to the menu board.
                next.leave_session();
                next.spectate = Some(SpectateStatus::Watching);
            }
            GameAction::SpectateEnded => {
                // The server releases every spectator of a finished match.
                next.spectate = Some(SpectateStatus::Ended);
                next.spectators.clear();
            }
            GameAction::MatchFinished(replay) => {
                next.finished_match = Some(FinishedMatch { replay });
            }
            GameAction::SetMatchList(matches) => next.match_list = Some(matches),
            GameAction::LoadReplay(payload) => {
                next.apply_init(*payload);
                next.spectate = Some(SpectateStatus::Watching);
            }
            GameAction::ShowReplayFrame(state) => next.state = *state,
            GameAction::SetBoardRotated(rotated) => next.rotated_180 = rotated,
            GameAction::SetReplayTime(at) => next.replay_time = Some(at),
        }
        next.phase = compute_phase(&next);
        next.into()
    }
}

impl GameStateReducer {
    /// Shows `error` to the player unless it is an expected, silent error.
    fn show_error(&mut self, error: GameError) {
        if !error.is_user_visible() {
            return;
        }
        let serial = self
            .error
            .as_ref()
            .map_or(0, |notice| notice.serial.wrapping_add(1));
        self.error = Some(ErrorNotice { error, serial });
    }

    /// Applies a disconnect or fatal notice.
    fn set_disconnect_notice(&mut self, notice: DisconnectNotice) {
        if notice.is_shown() {
            self.clear_victory_state();
        }
        self.connection = notice;
    }

    /// Updates latency and the server clock offset from a ping round trip.
    fn record_pong(&mut self, sent_at: u64, server_time: TimestampMs) {
        let now = js_sys::Date::now() as u64;
        let Some(round_trip) = now.checked_sub(sent_at) else {
            return;
        };
        self.ping_ms = round_trip;
        let sample = server_time.as_i64() - (sent_at + round_trip / 2) as i64;
        self.clock_offset_ms = if self.clock_offset_ms == 0 {
            sample
        } else {
            (self.clock_offset_ms as f64 * (1.0 - CLOCK_OFFSET_SMOOTHING)
                + sample as f64 * CLOCK_OFFSET_SMOOTHING) as i64
        };
    }

    /// Returns to watching the preview board, dropping the local session.
    fn reset_session(&mut self) {
        self.leave_session();
        self.state = self.menu_preview_state.clone().unwrap_or_default();
        self.replace_board();
        self.spectate = None;
        self.match_list = None;
    }

    /// Drops the local player, queue, and match result, keeping the shown board.
    fn leave_session(&mut self) {
        self.player_id = Some(PlayerId::nil());
        self.session_secret = None;
        self.move_unlock_at = None;
        self.premoves.clear();
        self.error = None;
        self.connection = DisconnectNotice::none();
        self.is_dead = false;
        self.clear_victory_state();
        self.queue_status = None;
        self.finished_match = None;
        self.camera_cue = None;
    }

    /// Clears all victory state so generic end overlays can be derived cleanly.
    pub(super) fn clear_victory_state(&mut self) {
        self.victory = None;
    }
}

/// Derives the session phase from authoritative server state plus the local death flag.
fn compute_phase(state: &GameStateReducer) -> ClientPhase {
    let Some(player_id) = state.active_player_id() else {
        return if state.is_spectating() {
            ClientPhase::Spectating
        } else if state.queue_status.is_some() {
            ClientPhase::Queued
        } else {
            ClientPhase::Menu
        };
    };
    if state.has_match_result() {
        return ClientPhase::Dead;
    }
    if state.state.king_of(player_id).is_some() {
        ClientPhase::Alive
    } else {
        ClientPhase::Joining
    }
}

/// Drops chat lines whose TTL and fade-out have both elapsed.
fn prune_expired_chat_lines(lines: &mut Vec<ChatLine>, now: TimestampMs, ttl_ms: u32) {
    let retention = ttl_ms.max(1) as i64 + CHAT_FADE_OUT_MS;
    lines.retain(|line| (now - line.sent_at).as_i64() < retention);
}
