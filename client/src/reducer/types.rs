use common::models::{GameModeClientConfig, GameState, PieceConfig, ShopConfig};
use common::protocol::{ClientMessage, GameError};
use common::types::{PieceId, PieceTypeId, PlayerId, SessionSecret, ShopId};
use glam::IVec2;
use std::collections::HashMap;

#[derive(Clone, PartialEq, Default, Debug)]
pub struct Pmove {
    pub piece_id: PieceId,
    pub target: IVec2,
    pub pending: bool,
    pub old_last_move_time: i64,
    pub old_cooldown_ms: i64,
}

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
    pub last_score: u64,
    pub last_kills: u32,
    pub last_captured: u32,
    pub last_survival_secs: u64,
    pub ping_ms: u64,
    pub fps: u32,
    pub disconnected: bool,
    pub fatal_error: bool,
    pub is_dead: bool,
    pub disconnected_title: Option<String>,
    pub disconnected_msg: Option<String>,
}

#[derive(Clone)]
pub struct MsgSender(pub tokio::sync::mpsc::UnboundedSender<ClientMessage>);

impl PartialEq for MsgSender {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
