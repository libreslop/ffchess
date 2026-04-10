use crate::types::{
    BoardCoord, BoardSize, ColorHex, DurationMs, ModeId, PieceId, PieceTypeId, PlayerId, Score,
    ShopId, TimestampMs,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Mutable piece state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Piece {
    pub id: PieceId,
    pub owner_id: Option<PlayerId>, // None for NPCs
    pub piece_type: PieceTypeId,
    pub position: BoardCoord,
    pub last_move_time: TimestampMs, // Milliseconds timestamp
    pub cooldown_ms: DurationMs,
}

/// Player state for the active match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub score: Score,
    pub kills: u32,
    pub pieces_captured: u32,
    pub join_time: TimestampMs,
    pub king_id: PieceId,
    pub color: ColorHex, // Hex color code
}

/// Shop state on the board.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shop {
    pub position: BoardCoord,
    pub uses_remaining: u32,
    pub shop_id: ShopId,
}

/// Snapshot of the board state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameState {
    pub players: HashMap<PlayerId, Player>,
    pub pieces: HashMap<PieceId, Piece>,
    pub shops: Vec<Shop>,
    pub board_size: BoardSize,
    pub mode_id: ModeId,
}

impl Default for GameState {
    /// Creates an empty game state using the default board and mode.
    fn default() -> Self {
        Self {
            players: HashMap::new(),
            pieces: HashMap::new(),
            shops: Vec::new(),
            board_size: BoardSize::default(),
            mode_id: ModeId::from("ffa"),
        }
    }
}
