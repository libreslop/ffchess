use glam::IVec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

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
            KitType::Standard => vec![
                PieceType::Pawn,
                PieceType::Pawn,
                PieceType::Knight,
                PieceType::Knight,
            ],
            KitType::Shield => vec![
                PieceType::Pawn,
                PieceType::Pawn,
                PieceType::Pawn,
                PieceType::Pawn,
                PieceType::Pawn,
                PieceType::Pawn,
            ],
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
    #[serde(skip_serializing, default)]
    pub last_move_time: i64, // Milliseconds timestamp
    #[serde(skip_serializing, default)]
    pub cooldown_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub id: Uuid,
    pub name: String,
    pub score: u64,
    pub kills: u32,
    pub pieces_captured: u32,
    pub join_time: i64,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CooldownConfig {
    pub pawn_base: i64,
    pub knight_base: i64,
    pub king_base: i64,
    pub bishop_base: i64,
    pub bishop_dist: f64,
    pub rook_base: i64,
    pub rook_dist: f64,
    pub queen_base: i64,
    pub queen_dist: f64,
}

impl Default for CooldownConfig {
    fn default() -> Self {
        Self {
            pawn_base: 1000,
            knight_base: 2000,
            king_base: 4000,
            bishop_base: 1200,
            bishop_dist: 400.0,
            rook_base: 1500,
            rook_dist: 400.0,
            queen_base: 2000,
            queen_dist: 500.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameState {
    pub players: HashMap<Uuid, Player>,
    pub pieces: HashMap<Uuid, Piece>,
    pub shops: Vec<Shop>,
    pub board_size: i32,
    pub cooldown_config: CooldownConfig,
    pub respawn_cooldown_ms: u64,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            players: HashMap::new(),
            pieces: HashMap::new(),
            shops: Vec::new(),
            board_size: 40,
            cooldown_config: CooldownConfig::default(),
            respawn_cooldown_ms: 5000,
        }
    }
}
