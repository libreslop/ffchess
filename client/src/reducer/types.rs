//! Reducer state types used by the client UI.

use common::models::{GameModeClientConfig, GameState, PieceConfig, ShopConfig};
use common::protocol::{ClientMessage, GameError, VictoryFocusTarget};
use common::types::{
    PieceId, PieceTypeId, PlayerCount, PlayerId, QueuePosition, Score, SessionSecret, ShopId,
};
use glam::IVec2;
use std::collections::HashMap;

/// Client-side requested move entry used for visuals until server state catches up.
#[derive(Clone, PartialEq, Default, Debug)]
pub struct Pmove {
    pub id: u64,
    pub piece_id: PieceId,
    pub target: IVec2,
    pub shop_item_index: Option<usize>,
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
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QueueStatus {
    pub position_in_queue: QueuePosition,
    pub queued_players: PlayerCount,
    pub required_players: PlayerCount,
}

impl QueueStatus {
    /// Returns how many additional players are needed to start the match.
    pub fn players_needed(&self) -> PlayerCount {
        self.required_players.saturating_sub(self.queued_players)
    }
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
    pub clock_offset_ms: i64,
    pub sync_interval_ms: u32,
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
pub struct MsgSender(pub tokio::sync::mpsc::Sender<ClientMessage>);

impl PartialEq for MsgSender {
    /// Treats all message senders as equal for Yew props diffing.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
