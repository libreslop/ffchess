use crate::models::hook::HookConfig;
use crate::models::summary::{GameModeClientConfig, KitSummary};
use crate::types::{
    DurationMs, ExprString, KitId, ModeId, PieceTypeId, PlayerCount, Score, ShopId,
};
use glam::IVec2;
use serde::{Deserialize, Serialize};

/// Rules and metadata for a piece type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PieceConfig {
    pub id: PieceTypeId,
    pub display_name: String,
    pub svg_path: String,
    pub score_value: Score,
    pub cooldown_ms: DurationMs,
    pub move_paths: Vec<Vec<IVec2>>,
    pub capture_paths: Vec<Vec<IVec2>>,
}

/// Shop item definition, including pricing and piece changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopItemConfig {
    pub display_name: String,
    pub price_expr: Option<ExprString>,
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
    pub color: Option<String>,
    #[serde(default)]
    pub auto_upgrade_single_item: bool,
    pub groups: Vec<ShopGroupConfig>,
    pub default_group: Option<ShopGroupConfig>,
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
    pub max_expr: ExprString,
}

/// How many shops of a given type should spawn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopCountConfig {
    pub shop_id: ShopId,
    pub count: u32,
}

/// Fixed shop placement at an absolute board coordinate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FixedShopConfig {
    pub shop_id: ShopId,
    pub position: [i32; 2],
}

impl FixedShopConfig {
    /// Returns the configured board coordinate for this shop.
    pub fn board_coord(&self) -> crate::types::BoardCoord {
        crate::types::BoardCoord(IVec2::new(self.position[0], self.position[1]))
    }
}

/// One piece placement in a queue-mode preset layout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueuePresetPieceConfig {
    pub piece_id: PieceTypeId,
    pub position: [i32; 2],
}

impl QueuePresetPieceConfig {
    /// Returns the configured board coordinate for this piece.
    pub fn board_coord(&self) -> crate::types::BoardCoord {
        crate::types::BoardCoord(IVec2::new(self.position[0], self.position[1]))
    }
}

/// Piece placement set for a single queued player slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueuePresetPlayerConfig {
    #[serde(default)]
    pub board_rotation_deg: i32,
    pub pieces: Vec<QueuePresetPieceConfig>,
}

/// Fixed queue spawn layout embedded in a mode config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueuePresetLayoutConfig {
    pub players: Vec<QueuePresetPlayerConfig>,
}

/// Full server mode configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameModeConfig {
    pub id: ModeId,
    pub display_name: String,
    pub max_players: PlayerCount,
    #[serde(default)]
    pub queue_players: PlayerCount,
    #[serde(default = "GameModeConfig::default_preview_switch_delay_ms")]
    pub preview_switch_delay_ms: DurationMs,
    pub board_size: ExprString,
    pub camera_pan_limit: ExprString,
    pub fog_of_war_radius: Option<ExprString>,
    pub respawn_cooldown_ms: DurationMs,
    pub npc_limits: Vec<NpcLimitConfig>,
    pub shop_counts: Vec<ShopCountConfig>,
    #[serde(default)]
    pub fixed_shops: Vec<FixedShopConfig>,
    pub kits: Vec<KitConfig>,
    #[serde(default)]
    pub queue_layout: Option<QueuePresetLayoutConfig>,
    pub hooks: Vec<HookConfig>,
}

impl GameModeConfig {
    /// Default delay before queue previews switch away from an ended game.
    pub fn default_preview_switch_delay_ms() -> DurationMs {
        DurationMs::from_millis(5000)
    }

    /// Returns the required queue size for matchmaking modes.
    pub fn queue_requirement(&self) -> Option<PlayerCount> {
        (self.queue_players.as_u32() >= 2).then_some(self.queue_players)
    }

    /// Builds a client-safe projection of this mode configuration.
    ///
    /// Returns a `GameModeClientConfig` with server-only fields stripped.
    pub fn to_client_config(&self) -> GameModeClientConfig {
        GameModeClientConfig {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            queue_players: self.queue_players,
            camera_pan_limit: self.camera_pan_limit.clone(),
            fog_of_war_radius: self.fog_of_war_radius.clone(),
            respawn_cooldown_ms: self.respawn_cooldown_ms,
            kits: self
                .kits
                .iter()
                .map(|k| KitSummary {
                    name: k.name.clone(),
                    description: k.description.clone(),
                    pieces: k.pieces.clone(),
                })
                .collect(),
        }
    }
}
