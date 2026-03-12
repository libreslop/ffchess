use crate::models::{GameState, KitType, Piece, PieceType, Player, Shop};
use glam::IVec2;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    InsufficientScore { needed: u64, have: u64 },
    NoSpaceNearby,
    Internal(String),
    Custom { title: String, message: String },
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PieceNotFound => write!(f, "Piece not found"),
            Self::NotYourPiece => write!(f, "Not your piece"),
            Self::OnCooldown => write!(f, "Piece is on cooldown"),
            Self::TargetFriendly => write!(f, "Target square is occupied by a friendly piece"),
            Self::InvalidMove => write!(f, "Invalid chess move"),
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
pub enum ClientMessage {
    Join {
        name: String,
        kit: KitType,
        player_id: Option<Uuid>,
    },
    MovePiece {
        piece_id: Uuid,
        target: IVec2,
    },
    BuyPiece {
        shop_pos: IVec2,
        piece_type: PieceType,
    },
    Ping(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    Init {
        player_id: Uuid,
        state: GameState,
    },
    UpdateState {
        players: Vec<Player>,
        pieces: Vec<Piece>,
        shops: Vec<Shop>,
        removed_pieces: Vec<Uuid>,
        removed_players: Vec<Uuid>,
        board_size: i32,
    },
    Error(GameError),
    GameOver {
        final_score: u64,
        kills: u32,
        pieces_captured: u32,
        time_survived_secs: u64,
    },
    Pong(u64),
}
