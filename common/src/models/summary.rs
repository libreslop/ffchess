use crate::types::{DurationMs, ExprString, KitId, ModeId, PlayerCount, Score};
use serde::{Deserialize, Serialize};

/// Minimal kit info sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KitSummary {
    pub name: KitId,
    pub description: String,
    pub pieces: Vec<crate::types::PieceTypeId>,
}

/// Client-safe mode configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameModeClientConfig {
    pub id: ModeId,
    pub display_name: String,
    #[serde(default)]
    pub queue_players: PlayerCount,
    pub camera_pan_limit: ExprString,
    pub fog_of_war_radius: Option<ExprString>,
    pub respawn_cooldown_ms: DurationMs,
    pub kits: Vec<KitSummary>,
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

impl ModeSummary {
    /// Returns true when this summary describes a queue-based mode.
    pub fn is_queue_mode(&self) -> bool {
        self.queue_players.as_u32() >= 2
    }
}
