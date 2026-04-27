//! Reducer actions and payloads for client state updates.

use crate::reducer::types::{Pmove, QueueStatus};
use common::models::{
    GameModeClientConfig, GameState, Piece, PieceConfig, Player, Shop, ShopConfig,
};
use common::protocol::{ChatLine, GameError, VictoryFocusTarget};
use common::types::{
    BoardSize, PieceId, PieceTypeId, PlayerId, Score, SessionSecret, ShopId, TimestampMs,
};
use std::collections::HashMap;

/// Initial game snapshot payload delivered on connect.
pub struct InitPayload {
    pub player_id: PlayerId,
    pub session_secret: SessionSecret,
    pub move_unlock_at: Option<TimestampMs>,
    pub state: GameState,
    pub mode: GameModeClientConfig,
    pub pieces: HashMap<PieceTypeId, PieceConfig>,
    pub shops: HashMap<ShopId, ShopConfig>,
    pub chat_room_key: String,
    pub chat_history: Vec<ChatLine>,
    pub sync_interval_ms: u32,
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
    PushChatLine(ChatLine),
    PruneExpiredChat {
        now: TimestampMs,
        ttl_ms: u32,
    },
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
    RemovePm(u64),
    ClearPm(PieceId),
    Pong(u64, common::types::TimestampMs),
    SetFPS(u32),
    SetDisconnected {
        disconnected: bool,
        is_fatal: bool,
        title: Option<String>,
        msg: Option<String>,
    },
    Reset,
}
