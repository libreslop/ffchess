pub use common::*;
use glam::IVec2;
use uuid::Uuid;

#[derive(Clone, PartialEq, Default, Debug)]
pub struct Pmove {
    pub piece_id: Uuid,
    pub target: IVec2,
    pub pending: bool,
    pub old_last_move_time: i64,
    pub old_cooldown_ms: i64,
}

#[derive(Clone, PartialEq, Default)]
pub struct GameStateReducer {
    pub state: GameState,
    pub player_id: Option<Uuid>,
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
