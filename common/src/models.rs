use crate::types::{ColorHex, KitId, ModeId, PieceId, PieceTypeId, PlayerId, ShopId};
use glam::IVec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Rules and metadata for a piece type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PieceConfig {
    pub id: PieceTypeId,
    pub display_name: String,
    pub char: char,
    pub score_value: u64,
    pub cooldown_ms: u64,
    pub move_paths: Vec<Vec<IVec2>>,
    pub capture_paths: Vec<Vec<IVec2>>,
}

/// Shop item definition, including pricing and piece changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopItemConfig {
    pub display_name: String,
    pub price_expr: String,
    pub replace_with: Option<PieceTypeId>,
    pub add_pieces: Vec<PieceTypeId>,
}

/// Grouping of shop items for specific piece types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopGroupConfig {
    pub applies_to: Vec<PieceTypeId>,
    pub items: Vec<ShopItemConfig>,
}

/// Shop configuration including item groups and defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopConfig {
    pub id: ShopId,
    pub display_name: String,
    pub default_uses: u32,
    pub color: Option<ColorHex>,
    pub groups: Vec<ShopGroupConfig>,
    pub default_group: ShopGroupConfig,
}

/// A player starting kit definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KitConfig {
    pub name: KitId,
    pub description: String,
    pub pieces: Vec<PieceTypeId>,
}

/// NPC spawn limit configuration per piece type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NpcLimitConfig {
    pub piece_id: PieceTypeId,
    pub max_expr: String,
}

/// How many shops of a given type should spawn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopCountConfig {
    pub shop_id: ShopId,
    pub count: u32,
}

/// Trigger-action hook for game events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookConfig {
    pub trigger: String,
    pub target_piece_id: PieceTypeId,
    pub action: String,
}

/// Minimal kit info sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KitSummary {
    pub name: KitId,
    pub description: String,
    pub pieces: Vec<PieceTypeId>,
}

/// Client-safe mode configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameModeClientConfig {
    pub id: ModeId,
    pub display_name: String,
    pub camera_pan_limit: String,
    pub fog_of_war_radius: String,
    pub respawn_cooldown_ms: u32,
    pub kits: Vec<KitSummary>,
}

/// Full server mode configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameModeConfig {
    pub id: ModeId,
    pub display_name: String,
    pub max_players: u32,
    pub board_size: String,
    pub camera_pan_limit: String,
    pub fog_of_war_radius: String,
    pub respawn_cooldown_ms: u32,
    pub npc_limits: Vec<NpcLimitConfig>,
    pub shop_counts: Vec<ShopCountConfig>,
    pub kits: Vec<KitConfig>,
    pub hooks: Vec<HookConfig>,
}

/// Mutable piece state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Piece {
    pub id: PieceId,
    pub owner_id: Option<PlayerId>, // None for NPCs
    pub piece_type: PieceTypeId,
    pub position: IVec2,
    pub last_move_time: i64, // Milliseconds timestamp
    pub cooldown_ms: i64,
}

/// Player state for the active match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub score: u64,
    pub kills: u32,
    pub pieces_captured: u32,
    pub join_time: i64,
    pub king_id: PieceId,
    pub color: ColorHex, // Hex color code
}

/// Shop state on the board.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shop {
    pub position: IVec2,
    pub uses_remaining: u32,
    pub shop_id: ShopId,
}

/// Snapshot of the board state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameState {
    pub players: HashMap<PlayerId, Player>,
    pub pieces: HashMap<PieceId, Piece>,
    pub shops: Vec<Shop>,
    pub board_size: i32,
    pub mode_id: ModeId,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            players: HashMap::new(),
            pieces: HashMap::new(),
            shops: Vec::new(),
            board_size: 40,
            mode_id: ModeId::from("ffa"),
        }
    }
}
