use serde::{Deserialize, Serialize};
use uuid::Uuid;
use glam::IVec2;
use crate::models::{PieceType, GameState, Player, Piece, Shop, KitType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Join { name: String, kit: KitType, player_id: Option<Uuid> },
    MovePiece { piece_id: Uuid, target: IVec2 },
    BuyPiece { shop_pos: IVec2, piece_type: PieceType },
    Ping(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    Init { 
        player_id: Uuid, 
        state: GameState 
    },
    UpdateState { 
        players: Vec<Player>,
        pieces: Vec<Piece>,
        shops: Vec<Shop>,
        removed_pieces: Vec<Uuid>,
        removed_players: Vec<Uuid>,
        board_size: i32,
    },
    Error(String),
    GameOver { 
        final_score: u64,
        kills: u32,
        pieces_captured: u32,
        time_survived_secs: u64,
    },
    Pong(u64),
}
