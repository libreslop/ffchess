use serde::{Deserialize, Serialize};
use uuid::Uuid;
use glam::IVec2;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PieceType {
    King,
    Queen,
    Rook,
    Bishop,
    Knight,
    Pawn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum KitType {
    Standard,
    Shield,
    Scout,
    Tank,
}

impl KitType {
    pub fn get_pieces(&self) -> Vec<PieceType> {
        match self {
            KitType::Standard => vec![PieceType::Pawn, PieceType::Pawn, PieceType::Knight, PieceType::Knight],
            KitType::Shield => vec![PieceType::Pawn, PieceType::Pawn, PieceType::Pawn, PieceType::Pawn, PieceType::Pawn, PieceType::Pawn],
            KitType::Scout => vec![PieceType::Pawn, PieceType::Bishop, PieceType::Bishop],
            KitType::Tank => vec![PieceType::Rook],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Piece {
    pub id: Uuid,
    pub owner_id: Option<Uuid>, // None for NPCs
    pub piece_type: PieceType,
    pub position: IVec2,
    pub last_move_time: i64, // Milliseconds timestamp
    pub cooldown_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub id: Uuid,
    pub name: String,
    pub score: u64,
    pub king_id: Uuid,
    pub color: String, // Hex color code
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShopType {
    Spawn,
    Upgrade,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shop {
    pub position: IVec2,
    pub uses_remaining: u32,
    pub shop_type: ShopType,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct GameState {
    pub players: HashMap<Uuid, Player>,
    pub pieces: HashMap<Uuid, Piece>,
    pub shops: Vec<Shop>,
    pub board_size: i32,
}
