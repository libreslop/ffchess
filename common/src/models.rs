//! Core domain models shared between server logic and client rendering.

use crate::types::{
    BoardSize, ColorHex, DurationMs, ExprString, KitId, ModeId, PieceId, PieceTypeId, PlayerCount,
    PlayerId, Score, ShopId, TimestampMs,
};
use glam::IVec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub price_expr: ExprString,
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
    pub max_expr: ExprString,
}

/// How many shops of a given type should spawn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShopCountConfig {
    pub shop_id: ShopId,
    pub count: u32,
}

/// Trigger-action hook for game events.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HookTrigger {
    #[serde(rename = "OnCapture")]
    OnCapture,
    #[serde(rename = "OnCapturePieceActive")]
    OnCapturePieceActive,
    #[serde(rename = "OnPlayerLeave")]
    OnPlayerLeave,
}

/// Effect applied when a hook trigger matches.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HookAction {
    #[serde(rename = "EliminateOwner")]
    EliminateOwner,
    #[serde(rename = "WinCapturer")]
    WinCapturer,
    #[serde(rename = "WinRemaining")]
    WinRemaining,
}

/// Camera focus policy for victory overlays produced by hooks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HookVictoryFocus {
    #[serde(rename = "CaptureSquare")]
    CaptureSquare,
    #[serde(rename = "KeepCurrent")]
    KeepCurrent,
}

/// Supported hook behaviors understood by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedHook {
    EliminateOwnerOnCapture,
    WinCapturerOnActiveCapture,
    WinRemainingOnPlayerLeave,
}

/// Trigger-action hook for game events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookConfig {
    pub trigger: HookTrigger,
    pub target_piece_id: Option<PieceTypeId>,
    #[serde(default)]
    pub players_left: Option<u32>,
    pub action: HookAction,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub victory_focus: Option<HookVictoryFocus>,
}

impl HookConfig {
    /// Returns the typed supported hook behavior for this config entry.
    pub fn supported_hook(&self) -> Option<SupportedHook> {
        match (self.trigger, self.action) {
            (HookTrigger::OnCapture, HookAction::EliminateOwner) => {
                Some(SupportedHook::EliminateOwnerOnCapture)
            }
            (HookTrigger::OnCapturePieceActive, HookAction::WinCapturer) => {
                Some(SupportedHook::WinCapturerOnActiveCapture)
            }
            (HookTrigger::OnPlayerLeave, HookAction::WinRemaining) => {
                Some(SupportedHook::WinRemainingOnPlayerLeave)
            }
            _ => None,
        }
    }

    /// Returns whether this hook targets the provided piece type, or all piece types.
    pub fn targets_piece(&self, piece_type_id: &PieceTypeId) -> bool {
        self.target_piece_id
            .as_ref()
            .is_none_or(|target_piece_id| target_piece_id == piece_type_id)
    }

    /// Returns the custom hook title or `default_title` when none is configured.
    pub fn victory_title_or(&self, default_title: &str) -> String {
        self.title
            .clone()
            .unwrap_or_else(|| default_title.to_string())
    }

    /// Returns the custom hook message or `default_message` when none is configured.
    pub fn victory_message_or(&self, default_message: &str) -> String {
        self.message
            .clone()
            .unwrap_or_else(|| default_message.to_string())
    }

    /// Returns the configured victory focus policy, or the default for `supported_hook`.
    pub fn victory_focus_or_default(&self, supported_hook: SupportedHook) -> HookVictoryFocus {
        self.victory_focus.unwrap_or(match supported_hook {
            SupportedHook::WinCapturerOnActiveCapture => HookVictoryFocus::CaptureSquare,
            SupportedHook::WinRemainingOnPlayerLeave => HookVictoryFocus::KeepCurrent,
            SupportedHook::EliminateOwnerOnCapture => HookVictoryFocus::KeepCurrent,
        })
    }
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
    #[serde(default)]
    pub queue_players: PlayerCount,
    pub camera_pan_limit: ExprString,
    pub fog_of_war_radius: ExprString,
    pub respawn_cooldown_ms: DurationMs,
    pub kits: Vec<KitSummary>,
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
    pub fog_of_war_radius: ExprString,
    pub respawn_cooldown_ms: DurationMs,
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

/// Summary info for mode selection screens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModeSummary {
    pub id: ModeId,
    pub display_name: String,
    pub players: PlayerCount,
    pub max_players: PlayerCount,
    pub queue_players: PlayerCount,
    pub respawn_cooldown_ms: DurationMs,
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
}

impl ModeSummary {
    /// Returns true when this summary describes a queue-based mode.
    pub fn is_queue_mode(&self) -> bool {
        self.queue_players.as_u32() >= 2
    }
}

impl GameModeConfig {
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
