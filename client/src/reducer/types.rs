//! Reducer state types used by the client UI.

use crate::premoves::PremoveQueue;
use crate::utils::DEFAULT_RESPAWN_COOLDOWN;
use common::models::{GameModeClientConfig, GameState, MatchSummary, Player};
use common::protocol::{ChatLine, ClientMessage, GameError, VictoryFocusTarget};
use common::rules::Catalog;
use common::types::{
    DurationMs, PlayerCount, PlayerId, QueuePosition, Score, SessionSecret, TimestampMs,
};
use std::rc::Rc;

/// High-level client session phase used for UI and camera behavior.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ClientPhase {
    #[default]
    Menu,
    Queued,
    Joining,
    Alive,
    Dead,
    /// Watching a board as a named spectator, without playing on it.
    Spectating,
}

/// Progress of watching a board as a named spectator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpectateStatus {
    /// The board is live; the server sends updates.
    Watching,
    /// The spectated match ended; the last board stays shown.
    Ended,
}

/// Queue state shown while waiting for a matchmaking game to start.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QueueStatus {
    pub position_in_queue: QueuePosition,
    pub queued_players: PlayerCount,
    pub required_players: PlayerCount,
    /// Local time the client started waiting in this queue.
    pub since: TimestampMs,
}

impl QueueStatus {
    /// Returns this update of a queue, keeping the start time of the `previous` status.
    pub fn continuing(self, previous: Option<&Self>) -> Self {
        Self {
            since: previous.map_or(self.since, |previous| previous.since),
            ..self
        }
    }

    /// Returns how long the client has been waiting at `now`.
    pub fn waited(&self, now: TimestampMs) -> DurationMs {
        now - self.since
    }

    /// Returns how many additional players are needed to start the match.
    pub fn players_needed(&self) -> PlayerCount {
        self.required_players.saturating_sub(self.queued_players)
    }
}

/// Statistics of the local player shown on the end screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MatchStats {
    pub score: Score,
    pub kills: u32,
    pub captured: u32,
    pub survival_secs: u64,
    /// Board rotation kept so the end screen keeps the player's perspective.
    pub board_rotation_deg: i32,
}

impl MatchStats {
    /// Captures the live stats of `player` at `now`.
    pub fn of(player: &Player, now: TimestampMs) -> Self {
        Self {
            score: player.score,
            kills: player.kills,
            captured: player.pieces_captured,
            survival_secs: (now - player.join_time).as_u64() / 1000,
            board_rotation_deg: player.board_rotation_deg,
        }
    }
}

/// Victory announced by the server.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Victory {
    pub title: String,
    pub message: String,
    pub focus_target: VictoryFocusTarget,
}

/// An error shown to the player, numbered so repeated errors show again.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ErrorNotice {
    pub error: GameError,
    pub serial: u32,
}

/// Chat room the client is in and its recent lines.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ChatLog {
    pub room_key: Option<String>,
    pub lines: Vec<ChatLine>,
}

/// Aggregated client game state and UI state.
#[derive(Clone, PartialEq, Default)]
pub struct GameStateReducer {
    pub state: GameState,
    /// Last board seen while not playing, restored when returning to the menu.
    pub menu_preview_state: Option<GameState>,
    pub mode: Option<GameModeClientConfig>,
    pub catalog: Rc<Catalog>,
    pub player_id: Option<PlayerId>,
    pub session_secret: Option<SessionSecret>,
    pub move_unlock_at: Option<TimestampMs>,
    pub error: Option<ErrorNotice>,
    pub premoves: PremoveQueue,
    pub stats: MatchStats,
    pub ping_ms: u64,
    pub clock_offset_ms: i64,
    pub fps: u32,
    pub disconnected: bool,
    pub fatal_error: bool,
    pub disconnected_title: Option<String>,
    pub disconnected_msg: Option<String>,
    pub is_dead: bool,
    pub victory: Option<Victory>,
    pub queue_status: Option<QueueStatus>,
    pub phase: ClientPhase,
    pub chat: ChatLog,
    pub spectate: Option<SpectateStatus>,
    /// Names of the named spectators of the shown board.
    pub spectators: Vec<String>,
    /// Running matches offered for spectating, once the server listed them.
    pub match_list: Option<Vec<MatchSummary>>,
}

impl GameStateReducer {
    /// Returns the local player id when it represents an active session player.
    pub fn active_player_id(&self) -> Option<PlayerId> {
        self.player_id.filter(|player_id| !player_id.is_nil())
    }

    /// Returns the local player id, or the nil id while spectating.
    pub fn viewer_id(&self) -> PlayerId {
        self.player_id.unwrap_or_else(PlayerId::nil)
    }

    /// Returns true after the server announced a victory for the local player.
    pub fn is_victory(&self) -> bool {
        self.victory.is_some()
    }

    /// Returns true while an end-of-match result is shown.
    pub fn has_match_result(&self) -> bool {
        self.is_dead || self.is_victory()
    }

    /// Returns true while the client plays, queues, or spectates, so moving elsewhere must tell
    /// the server.
    pub fn has_session(&self) -> bool {
        self.phase == ClientPhase::Alive
            || self.queue_status.is_some()
            || self.active_player_id().is_some()
            || self.spectate.is_some()
    }

    /// Returns true while the client watches a board as a named spectator, ended or not.
    pub fn is_spectating(&self) -> bool {
        self.spectate.is_some()
    }

    /// Returns true once the spectated match ended.
    pub fn spectate_ended(&self) -> bool {
        self.spectate == Some(SpectateStatus::Ended)
    }

    /// Returns true when the mode matches players through a queue of separate matches.
    pub fn is_queue_mode(&self) -> bool {
        self.mode
            .as_ref()
            .is_some_and(|mode| mode.queue_players.as_u32() >= 2)
    }

    /// Returns true while the local player still has a place on the board, winners included.
    pub fn is_on_board(&self) -> bool {
        self.active_player_id()
            .is_some_and(|player_id| self.state.players.contains_key(&player_id))
    }

    /// Returns how long the mode makes players wait before joining again.
    pub fn respawn_cooldown(&self) -> DurationMs {
        self.mode
            .as_ref()
            .map_or(DEFAULT_RESPAWN_COOLDOWN, |mode| mode.respawn_cooldown_ms)
    }

    /// Returns true when the board should be viewed rotated by 180 degrees.
    pub fn board_rotated_180(&self) -> bool {
        let Some(player_id) = self.active_player_id() else {
            return false;
        };
        let rotation = self
            .state
            .players
            .get(&player_id)
            .map_or(self.stats.board_rotation_deg, |player| {
                player.board_rotation_deg
            });
        rotation.rem_euclid(360) == 180
    }

    /// Returns true when the mode locks the camera in place.
    pub fn camera_movement_disabled(&self) -> bool {
        self.mode
            .as_ref()
            .is_some_and(|mode| mode.disable_camera_movement)
    }

    /// Returns the current time on the server clock.
    pub fn server_now(&self) -> TimestampMs {
        TimestampMs::from_millis(js_sys::Date::now() as i64 + self.clock_offset_ms)
    }
}

/// Channel sender wrapper for client messages.
#[derive(Clone)]
pub struct MsgSender(pub tokio::sync::mpsc::Sender<ClientMessage>);

impl MsgSender {
    /// Queues a message for the server, logging when the outgoing buffer is unavailable.
    pub fn send(&self, message: ClientMessage) -> bool {
        let sent = self.0.try_send(message).is_ok();
        if !sent {
            web_sys::console::error_1(&"Failed to queue a message for the server".into());
        }
        sent
    }
}

impl PartialEq for MsgSender {
    /// Treats all message senders as equal for Yew props diffing.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
