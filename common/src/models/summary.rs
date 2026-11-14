use crate::models::config::JoinCameraCenterConfig;
use crate::types::{ColorHex, DurationMs, ExprString, KitId, ModeId, PlayerCount, TimestampMs};
use educe::Educe;
use serde::{Deserialize, Serialize};

/// Minimal kit info sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KitSummary {
    pub name: KitId,
    pub description: String,
    pub pieces: Vec<crate::types::PieceTypeId>,
}

/// Client-safe mode configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Educe)]
#[serde(default)]
#[educe(Default)]
pub struct GameModeClientConfig {
    #[educe(Default = ModeId::from(""))]
    pub id: ModeId,
    #[educe(Default = String::new())]
    pub display_name: String,
    #[educe(Default = PlayerCount::zero())]
    pub queue_players: PlayerCount,
    #[educe(Default = DurationMs::zero())]
    pub queue_countdown_ms: DurationMs,
    #[educe(Default = ExprString::from("0"))]
    pub camera_pan_limit: ExprString,
    #[educe(Default = None)]
    pub fog_of_war_radius: Option<ExprString>,
    #[educe(Default = true)]
    pub show_scoreboard: bool,
    #[educe(Default = JoinCameraCenterConfig::Piece {
        piece_id: crate::types::PieceTypeId::from("king")
    })]
    pub join_camera_center: JoinCameraCenterConfig,
    #[educe(Default = false)]
    pub disable_camera_movement: bool,
    #[educe(Default = DurationMs::zero())]
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

/// A player of a running match, as listed to prospective spectators.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatchPlayer {
    pub name: String,
    pub color: ColorHex,
}

/// A running private match of a queue mode that can be spectated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatchSummary {
    /// Instance id of the match, used as its [`crate::protocol::SpectateTarget::Match`].
    pub id: ModeId,
    pub players: Vec<MatchPlayer>,
    pub spectators: PlayerCount,
    /// Server time the match started.
    pub started_at: TimestampMs,
}
