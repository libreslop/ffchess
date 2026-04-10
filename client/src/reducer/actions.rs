//! Reducer actions and payloads for client state updates.

use crate::reducer::types::{Pmove, QueueStatus};
use common::models::{
    GameModeClientConfig, GameState, Piece, PieceConfig, Player, Shop, ShopConfig,
};
use common::protocol::{GameError, VictoryFocusTarget};
use common::types::{BoardSize, PieceId, PieceTypeId, PlayerId, Score, SessionSecret, ShopId};
use std::collections::HashMap;

/// Initial game snapshot payload delivered on connect.
pub struct InitPayload {
    pub player_id: PlayerId,
    pub session_secret: SessionSecret,
    pub state: GameState,
    pub mode: GameModeClientConfig,
    pub pieces: HashMap<PieceTypeId, PieceConfig>,
    pub shops: HashMap<ShopId, ShopConfig>,
}

/// Incremental world update payload.
pub struct UpdateStatePayload {
    pub players: Vec<Player>,
    pub pieces: Vec<Piece>,
    pub shops: Vec<Shop>,
    pub removed_pieces: Vec<PieceId>,
    pub removed_players: Vec<PlayerId>,
    pub board_size: BoardSize,
}

/// Actions that drive the client reducer state machine.
pub enum GameAction {
    SetInit(Box<InitPayload>),
    SetQueueStatus(QueueStatus),
    UpdateState(Box<UpdateStatePayload>),
    SetError(GameError),
    SetVictory {
        title: String,
        msg: String,
        focus_target: VictoryFocusTarget,
    },
    GameOver {
        final_score: Score,
        kills: u32,
        pieces_captured: u32,
        time_survived_secs: u64,
    },
    AddPmove(Pmove),
    Pong(u64),
    SetFPS(u32),
    SetDisconnected {
        disconnected: bool,
        is_fatal: bool,
        title: Option<String>,
        msg: Option<String>,
    },
    Reset,
}
