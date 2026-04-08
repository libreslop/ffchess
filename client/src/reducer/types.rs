//! Reducer state types used by the client UI.

use common::models::{GameModeClientConfig, GameState, PieceConfig, ShopConfig};
use common::protocol::{ClientMessage, GameError, VictoryFocusTarget};
use common::types::{
    DurationMs, PieceId, PieceTypeId, PlayerId, Score, SessionSecret, ShopId, TimestampMs,
};
use glam::IVec2;
use std::collections::HashMap;

/// Client-side pending move entry for prediction and reconciliation.
#[derive(Clone, PartialEq, Default, Debug)]
pub struct Pmove {
    pub piece_id: PieceId,
    pub target: IVec2,
    pub pending: bool,
    pub old_last_move_time: TimestampMs,
    pub old_cooldown_ms: DurationMs,
}

/// High-level client session phase used for UI and camera behavior.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ClientPhase {
    #[default]
    Menu,
    Queued,
    Joining,
    Alive,
    Dead,
}

/// Queue state shown while waiting for a matchmaking game to start.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QueuePosition(u32);

impl QueuePosition {
    /// Wraps a queue position value.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

/// Number of players for queue-state accounting.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QueuePlayerCount(u32);

impl QueuePlayerCount {
    /// Wraps a queue player-count value.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the player count as `u32`.
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Queue state shown while waiting for a matchmaking game to start.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QueueStatus {
    pub position_in_queue: QueuePosition,
    pub queued_players: QueuePlayerCount,
    pub required_players: QueuePlayerCount,
}

/// Aggregated client game state and UI state.
#[derive(Clone, PartialEq, Default)]
pub struct GameStateReducer {
    pub state: GameState,
    pub menu_preview_state: Option<GameState>,
    pub mode: Option<GameModeClientConfig>,
    pub piece_configs: HashMap<PieceTypeId, PieceConfig>,
    pub shop_configs: HashMap<ShopId, ShopConfig>,
    pub player_id: Option<PlayerId>,
    pub session_secret: Option<SessionSecret>,
    pub error: Option<GameError>,
    pub pm_queue: Vec<Pmove>,
    pub last_score: Score,
    pub last_kills: u32,
    pub last_captured: u32,
    pub last_survival_secs: u64,
    pub ping_ms: u64,
    pub fps: u32,
    pub disconnected: bool,
    pub fatal_error: bool,
    pub is_dead: bool,
    pub is_victory: bool,
    pub queue_status: Option<QueueStatus>,
    pub phase: ClientPhase,
    pub disconnected_title: Option<String>,
    pub disconnected_msg: Option<String>,
    pub victory_title: Option<String>,
    pub victory_msg: Option<String>,
    pub victory_focus_target: VictoryFocusTarget,
}

impl GameStateReducer {
    /// Returns the local player id when it represents an active session player.
    pub fn active_player_id(&self) -> Option<PlayerId> {
        self.player_id
            .filter(|player_id| *player_id != PlayerId::nil())
    }
}

/// Channel sender wrapper for client messages.
#[derive(Clone)]
pub struct MsgSender(pub tokio::sync::mpsc::UnboundedSender<ClientMessage>);

impl PartialEq for MsgSender {
    /// Treats all message senders as equal for Yew props diffing.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
