use crate::types::Score;
use serde::{Deserialize, Serialize};
use std::fmt;

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
