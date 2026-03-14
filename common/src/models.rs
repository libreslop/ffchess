use glam::IVec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PieceConfig {
    pub id: String,
    pub display_name: String,
    pub char: char,
    pub score_value: u64,
    pub cooldown_ms: u64,
    pub move_paths: Vec<Vec<IVec2>>,
    pub capture_paths: Vec<Vec<IVec2>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopItemConfig {
    pub display_name: String,
    pub price_expr: String,
    pub replace_with: Option<String>,
    pub add_pieces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopGroupConfig {
    #[serde(default)]
    pub applies_to: Vec<String>,
    pub items: Vec<ShopItemConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopConfig {
    pub id: String,
    pub display_name: String,
    pub default_uses: u32,
    pub groups: Vec<ShopGroupConfig>,
    pub default_group: ShopGroupConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KitConfig {
    pub name: String,
    pub description: String,
    pub pieces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NpcLimitConfig {
    pub piece_id: String,
    pub max_expr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopCountConfig {
    pub shop_id: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookConfig {
    pub trigger: String,
    pub target_piece_id: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KitSummary {
    pub name: String,
    pub description: String,
    pub pieces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameModeClientConfig {
    pub id: String,
    pub display_name: String,
    pub camera_pan_limit: String,
    pub fog_of_war_radius: String,
    pub respawn_cooldown_ms: u32,
    pub kits: Vec<KitSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameModeConfig {
    pub id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Piece {
    pub id: Uuid,
    pub owner_id: Option<Uuid>, // None for NPCs
    pub piece_type: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Shop {
    pub position: IVec2,
    pub uses_remaining: u32,
    pub shop_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameState {
    pub players: HashMap<Uuid, Player>,
    pub pieces: HashMap<Uuid, Piece>,
    pub shops: Vec<Shop>,
    pub board_size: i32,
    pub mode_id: String,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            players: HashMap::new(),
            pieces: HashMap::new(),
            shops: Vec::new(),
            board_size: 40,
            mode_id: "ffa".to_string(),
        }
    }
}
