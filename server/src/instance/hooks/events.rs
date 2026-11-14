//! Gameplay events collected between ticks for hook resolution.

use common::types::{BoardCoord, PieceTypeId, PlayerId};

/// A captured piece.
#[derive(Debug, Clone)]
pub(super) struct CaptureEvent {
    pub(super) capturer_id: Option<PlayerId>,
    pub(super) captured_piece_type: PieceTypeId,
    pub(super) captured_owner_id: Option<PlayerId>,
    pub(super) captured_position: BoardCoord,
}

/// A player whose king was captured.
#[derive(Debug, Clone)]
pub(super) struct KillEvent {
    pub(super) victim_name: String,
    /// Name of the capturing player, or `None` when an NPC took the king.
    pub(super) killer_name: Option<String>,
}

/// Events observed since the last resolved tick.
#[derive(Debug, Default)]
pub(in crate::instance) struct HookEvents {
    pub(super) captures: Vec<CaptureEvent>,
    pub(super) player_left: bool,
    pub(super) joins: Vec<String>,
    pub(super) disconnects: Vec<String>,
    pub(super) kills: Vec<KillEvent>,
    pub(super) countdowns: Vec<u32>,
    pub(super) game_started: bool,
    pub(in crate::instance) spectator_joins: Vec<String>,
    pub(in crate::instance) spectator_leaves: Vec<String>,
}

impl HookEvents {
    /// Returns true when nothing hook-relevant happened.
    pub(super) fn is_empty(&self) -> bool {
        self.captures.is_empty()
            && !self.player_left
            && self.joins.is_empty()
            && self.disconnects.is_empty()
            && self.kills.is_empty()
            && self.countdowns.is_empty()
            && !self.game_started
            && self.spectator_joins.is_empty()
            && self.spectator_leaves.is_empty()
    }

    /// Returns the owners of captured pieces whose type matches `matches`.
    pub(super) fn captured_owners(&self, matches: impl Fn(&PieceTypeId) -> bool) -> Vec<PlayerId> {
        self.captures
            .iter()
            .filter(|event| matches(&event.captured_piece_type))
            .filter_map(|event| event.captured_owner_id)
            .collect()
    }

    /// Returns the first capture whose piece type matches `matches`.
    pub(super) fn first_capture(
        &self,
        matches: impl Fn(&PieceTypeId) -> bool,
    ) -> Option<&CaptureEvent> {
        self.captures
            .iter()
            .find(|event| matches(&event.captured_piece_type))
    }
}
