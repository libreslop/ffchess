//! Wire protocol types shared between server and client.

use crate::models::{
    GameModeClientConfig, GameState, Piece, PieceConfig, Player, Shop, ShopConfig,
};
use crate::types::{
    BoardSize, KitId, PieceId, PieceTypeId, PlayerCount, PlayerId, QueuePosition, Score,
    SessionSecret, ShopId,
};
use glam::IVec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Camera target behavior for a victory overlay.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum VictoryFocusTarget {
    #[default]
    KeepCurrent,
    BoardPosition(IVec2),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Enumerates gameplay errors sent from the server to clients.
pub enum GameError {
    PieceNotFound,
    NotYourPiece,
    OnCooldown,
    TargetFriendly,
    InvalidMove,
    PathBlocked,
    NoPieceOnShop,
    KingRestrictedShop,
    ShopNotFound,
    ShopDepleted,
    PlayerNotFound,
    InsufficientScore { needed: Score, have: Score },
    NoSpaceNearby,
    Internal(String),
    Custom { title: String, message: String },
}

impl fmt::Display for GameError {
    /// Formats a user-facing error message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PieceNotFound => write!(f, "Piece not found"),
            Self::NotYourPiece => write!(f, "Not your piece"),
            Self::OnCooldown => write!(f, "Piece is on cooldown"),
            Self::TargetFriendly => write!(f, "Target square is occupied by a friendly piece"),
            Self::InvalidMove => write!(f, "Invalid move"),
            Self::PathBlocked => write!(f, "Path is blocked by another piece"),
            Self::NoPieceOnShop => write!(f, "No piece of yours on the shop square"),
            Self::KingRestrictedShop => write!(f, "The King can only recruit Pawns"),
            Self::ShopNotFound => write!(f, "Shop not found at this position"),
            Self::ShopDepleted => write!(f, "This shop has been depleted"),
            Self::PlayerNotFound => write!(f, "Player not found"),
            Self::InsufficientScore { needed, have } => {
                write!(f, "Insufficient score. Need {}, have {}", needed, have)
            }
            Self::NoSpaceNearby => write!(f, "No free space nearby"),
            Self::Internal(s) => write!(f, "Internal error: {}", s),
            Self::Custom { title, message } => write!(f, "{}: {}", title, message),
        }
    }
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
        target: IVec2,
    },
    BuyPiece {
        shop_pos: IVec2,
        item_index: usize,
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
    Pong(u64),
}
