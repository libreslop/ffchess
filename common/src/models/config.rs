use crate::models::hook::HookConfig;
use crate::models::state::Piece;
use crate::models::summary::{GameModeClientConfig, KitSummary};
use crate::types::{
    BoardCoord, BoardSize, DurationMs, ExprString, ExprVars, KitId, ModeId, PieceTypeId,
    PlayerCount, Score, ShopId,
};
use educe::Educe;
use glam::IVec2;
use serde::{Deserialize, Serialize};

/// Rules and metadata for a piece type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShopItemConfig {
    pub display_name: String,
    pub price_expr: Option<ExprString>,
    pub replace_with: Option<PieceTypeId>,
    pub add_pieces: Vec<PieceTypeId>,
}

impl ShopItemConfig {
    /// Returns the item price for the buyer described by `vars`.
    ///
    /// Items without a price expression are free; negative results clamp to zero.
    pub fn price(&self, vars: &ExprVars) -> Score {
        self.price_expr
            .as_ref()
            .map(|expr| Score::from(expr.evaluate(vars).max(0.0) as u64))
            .unwrap_or_else(Score::zero)
    }
}

/// Grouping of shop items for specific piece types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShopGroupConfig {
    #[serde(default)]
    pub applies_to: Vec<PieceTypeId>,
    pub items: Vec<ShopItemConfig>,
}

/// Shop configuration including item groups and defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShopConfig {
    pub id: ShopId,
    pub display_name: String,
    pub default_uses: u32,
    pub color: Option<String>,
    pub focus_color: Option<String>,
    #[serde(default)]
    pub auto_upgrade_single_item: bool,
    pub groups: Vec<ShopGroupConfig>,
    pub default_group: Option<ShopGroupConfig>,
}

impl ShopConfig {
    /// Resolves the item group offered to `piece` standing on this shop.
    ///
    /// Falls back to the default group when no group lists the piece type.
    pub fn group_for(&self, piece: Option<&Piece>) -> Option<&ShopGroupConfig> {
        piece
            .and_then(|piece| {
                self.groups
                    .iter()
                    .find(|group| group.applies_to.contains(&piece.piece_type))
            })
            .or(self.default_group.as_ref())
    }

    /// Returns the item bought automatically when `piece` lands on this shop.
    ///
    /// Only shops flagged `auto_upgrade_single_item` whose resolved group has exactly one
    /// item qualify.
    pub fn auto_item_for(&self, piece: &Piece) -> Option<&ShopItemConfig> {
        if !self.auto_upgrade_single_item {
            return None;
        }
        match self.group_for(Some(piece))?.items.as_slice() {
            [item] => Some(item),
            _ => None,
        }
    }

    /// Returns the items a player can pick manually with `piece` on this shop.
    ///
    /// Returns `None` when there is nothing to choose, including auto-upgrade shops.
    pub fn manual_items_for(&self, piece: &Piece) -> Option<&[ShopItemConfig]> {
        if self.auto_item_for(piece).is_some() {
            return None;
        }
        let items = self.group_for(Some(piece))?.items.as_slice();
        (!items.is_empty()).then_some(items)
    }
}

/// A player starting kit definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KitConfig {
    pub name: KitId,
    pub description: String,
    pub pieces: Vec<PieceTypeId>,
}

impl KitConfig {
    /// Returns true when the kit contains a king, which every army requires.
    pub fn has_king(&self) -> bool {
        self.pieces.iter().any(PieceTypeId::is_king)
    }

    /// Returns the client-facing summary of this kit.
    pub fn summary(&self) -> KitSummary {
        KitSummary {
            name: self.name.clone(),
            description: self.description.clone(),
            pieces: self.pieces.clone(),
        }
    }
}

/// NPC spawn limit configuration per piece type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NpcLimitConfig {
    pub piece_id: PieceTypeId,
    pub max_expr: ExprString,
}

impl NpcLimitConfig {
    /// Returns how many NPCs of this type may exist with `player_count` players present.
    pub fn max_count(&self, player_count: usize) -> usize {
        let vars = ExprVars::new().with("player_count", player_count as f64);
        self.max_expr.evaluate(&vars).max(0.0) as usize
    }
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
    pub fn board_coord(&self) -> BoardCoord {
        BoardCoord::new(self.position[0], self.position[1])
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
    pub fn board_coord(&self) -> BoardCoord {
        BoardCoord::new(self.position[0], self.position[1])
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

/// Join-time camera focus target for a mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum JoinCameraCenterConfig {
    Piece { piece_id: PieceTypeId },
    Position { position: [f64; 2] },
}

/// Full server mode configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Educe)]
#[serde(default)]
#[educe(Default)]
pub struct GameModeConfig {
    #[educe(Default = ModeId::from(""))]
    pub id: ModeId,
    #[educe(Default = String::new())]
    pub display_name: String,
    #[educe(Default = PlayerCount::zero())]
    pub max_players: PlayerCount,
    #[educe(Default = PlayerCount::zero())]
    pub queue_players: PlayerCount,
    #[educe(Default = DurationMs::from_millis(5000))]
    pub preview_switch_delay_ms: DurationMs,
    #[educe(Default = DurationMs::zero())]
    pub queue_countdown_ms: DurationMs,
    #[educe(Default = ExprString::from("1"))]
    pub board_size: ExprString,
    #[educe(Default = ExprString::from("0"))]
    pub camera_pan_limit: ExprString,
    #[educe(Default = None)]
    pub fog_of_war_radius: Option<ExprString>,
    #[educe(Default = true)]
    pub show_scoreboard: bool,
    #[educe(Default = JoinCameraCenterConfig::Piece {
        piece_id: PieceTypeId::from("king")
    })]
    pub join_camera_center: JoinCameraCenterConfig,
    #[educe(Default = false)]
    pub disable_camera_movement: bool,
    #[educe(Default = DurationMs::zero())]
    pub respawn_cooldown_ms: DurationMs,
    pub npc_limits: Vec<NpcLimitConfig>,
    pub shop_counts: Vec<ShopCountConfig>,
    pub fixed_shops: Vec<FixedShopConfig>,
    pub kits: Vec<KitConfig>,
    pub queue_layout: Option<QueuePresetLayoutConfig>,
    pub hooks: Vec<HookConfig>,
}

impl GameModeConfig {
    /// Returns the required queue size for matchmaking modes.
    pub fn queue_requirement(&self) -> Option<PlayerCount> {
        (self.queue_players.as_u32() >= 2).then_some(self.queue_players)
    }

    /// Returns the board size this mode wants for `player_count` players.
    pub fn board_size_for(&self, player_count: usize) -> BoardSize {
        let vars = ExprVars::new().with("player_count", player_count as f64);
        BoardSize::from(self.board_size.evaluate(&vars).trunc() as i32)
    }

    /// Returns the kit with the given name.
    pub fn kit(&self, name: &KitId) -> Option<&KitConfig> {
        self.kits.iter().find(|kit| &kit.name == name)
    }

    /// Builds a client-safe projection of this mode configuration.
    pub fn to_client_config(&self) -> GameModeClientConfig {
        GameModeClientConfig {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            queue_players: self.queue_players,
            queue_countdown_ms: self.queue_countdown_ms,
            camera_pan_limit: self.camera_pan_limit.clone(),
            fog_of_war_radius: self.fog_of_war_radius.clone(),
            show_scoreboard: self.show_scoreboard,
            join_camera_center: self.join_camera_center.clone(),
            disable_camera_movement: self.disable_camera_movement,
            respawn_cooldown_ms: self.respawn_cooldown_ms,
            kits: self.kits.iter().map(KitConfig::summary).collect(),
        }
    }
}
