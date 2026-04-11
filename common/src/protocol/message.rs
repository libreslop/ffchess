use crate::models::{
    GameModeClientConfig, GameState, Piece, PieceConfig, Player, Shop, ShopConfig,
};
use crate::protocol::error::GameError;
use crate::types::{
    BoardCoord, BoardSize, KitId, PieceId, PieceTypeId, PlayerCount, PlayerId, QueuePosition,
    Score, SessionSecret, ShopId, TimestampMs,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Camera target behavior for a victory overlay.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum VictoryFocusTarget {
    #[default]
    KeepCurrent,
    BoardPosition(BoardCoord),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Messages sent from the client to the server.
pub enum ClientMessage {
    Join {
        name: String,
        kit_name: KitId,
        player_id: Option<PlayerId>,
        session_secret: Option<SessionSecret>,
    },
    Leave,
    MovePiece {
        piece_id: PieceId,
        target: BoardCoord,
    },
    BuyPiece {
        shop_pos: BoardCoord,
        item_index: usize,
    },
    ClearPremoves {
        piece_id: PieceId,
    },
    SetPreviewDefault {
        enabled: bool,
    },
    Ping(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Messages sent from the server to the client.
pub enum ServerMessage {
    Init {
        player_id: PlayerId,
        session_secret: SessionSecret,
        state: Box<GameState>,
        mode: GameModeClientConfig,
        pieces: HashMap<PieceTypeId, PieceConfig>,
        shops: HashMap<ShopId, ShopConfig>,
        sync_interval_ms: u32,
    },
    UpdateState {
        players: Vec<Player>,
        pieces: Vec<Piece>,
        shops: Vec<Shop>,
        removed_pieces: Vec<PieceId>,
        removed_players: Vec<PlayerId>,
        board_size: BoardSize,
    },
    Error(GameError),
    Victory {
        title: String,
        message: String,
        focus_target: VictoryFocusTarget,
    },
    QueueState {
        position_in_queue: QueuePosition,
        queued_players: PlayerCount,
        required_players: PlayerCount,
    },
    GameOver {
        final_score: Score,
        kills: u32,
        pieces_captured: u32,
        time_survived_secs: u64,
    },
    Pong(u64, TimestampMs),
}
