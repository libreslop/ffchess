//! Reducer state types used by the client UI.

use common::models::{GameModeClientConfig, GameState, PieceConfig, ShopConfig};
use common::protocol::{ClientMessage, GameError};
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
    Joining,
    Alive,
    Dead,
}

/// Aggregated client game state and UI state.
#[derive(Clone, PartialEq, Default)]
pub struct GameStateReducer {
    pub state: GameState,
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
    pub phase: ClientPhase,
    pub disconnected_title: Option<String>,
    pub disconnected_msg: Option<String>,
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
