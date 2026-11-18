use crate::models::camera::StartingCameraConfig;
use crate::types::{
    ColorHex, DurationMs, ExprString, ExprVars, KitId, ModeId, PieceTypeId, PlayerCount,
    TimestampMs,
};
use educe::Educe;
use serde::{Deserialize, Serialize};

/// Minimal kit info sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KitSummary {
    pub name: KitId,
    pub description: String,
    pub pieces: Vec<PieceTypeId>,
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
    pub starting_camera: StartingCameraConfig,
    #[educe(Default = false)]
    pub disable_camera_movement: bool,
    #[educe(Default = DurationMs::zero())]
    pub respawn_cooldown_ms: DurationMs,
    pub kits: Vec<KitSummary>,
}

impl GameModeClientConfig {
    /// Returns true when the mode matches players through a queue of separate matches.
    pub fn is_queue_mode(&self) -> bool {
        self.queue_players.needs_queue()
    }

    /// Returns the fog-of-war radius, in squares, around the army of a player owning
    /// `piece_count` pieces, or `None` when the mode has no fog of war.
    pub fn fog_radius(&self, piece_count: usize) -> Option<f64> {
        self.fog_of_war_radius
            .as_ref()
            .map(|radius| radius.evaluate(&army_vars(piece_count)))
    }

    /// Returns how far, in squares, the camera may stray from the king of a player owning
    /// `piece_count` pieces.
    pub fn pan_limit(&self, piece_count: usize) -> f64 {
        let fog_radius = self.fog_radius(piece_count).unwrap_or(0.0);
        let vars = army_vars(piece_count).with("fog_of_war_radius", fog_radius);
        self.camera_pan_limit.evaluate(&vars)
    }
}

/// Returns the expression variables describing the army of a player owning `piece_count`
/// pieces.
fn army_vars(piece_count: usize) -> ExprVars {
    ExprVars::new().with("player_piece_count", piece_count)
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
        self.queue_players.needs_queue()
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
