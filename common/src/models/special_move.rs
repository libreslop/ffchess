//! Config of special moves: moves with extra conditions and side effects on other pieces.

use crate::types::{ExprString, PieceTypeId};
use glam::IVec2;
use serde::{Deserialize, Serialize};

/// A move a piece may make beyond its `move_paths` and `capture_paths`.
///
/// The `type` field of the config selects the variant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SpecialMove {
    /// The mover lands on its target while partner pieces act alongside it (castling,
    /// en passant, swaps).
    PartneredMove(PartneredMove),
}

/// A move of the mover together with actions on any number of partner pieces.
///
/// Every offset is relative to the mover's square before the move. The move is legal only
/// when the mover's condition holds, every square requirement holds, and every partner is
/// present and satisfies its own condition. All conditions see the board before the move.
/// All pieces then move at the same instant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartneredMove {
    /// Where the mover lands; selecting this square triggers the move.
    pub target: IVec2,
    /// Condition on the mover, evaluated with the `self_*` variables.
    #[serde(default)]
    pub condition: Option<ExprString>,
    /// Conditions on board squares, such as being empty and not under attack.
    #[serde(default)]
    pub requirements: Vec<SquareRequirement>,
    /// Piece type the mover turns into; it keeps its type when unset.
    #[serde(default)]
    pub becomes: Option<PieceTypeId>,
    /// Pieces that must be present, and what happens to them.
    #[serde(default)]
    pub partners: Vec<Partner>,
}

/// A condition a [`PartneredMove`] places on one board square.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SquareRequirement {
    /// The square checked.
    pub at: IVec2,
    /// Condition on the square, evaluated with the `self_*` and `square_*` variables.
    pub condition: ExprString,
}

/// A piece a [`PartneredMove`] requires, and what the move does to it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Partner {
    /// Square the partner stands on.
    pub at: IVec2,
    /// Whose piece the partner must be.
    pub owner: PartnerOwner,
    /// Accepted piece types; any type is accepted when empty.
    #[serde(default)]
    pub types: Vec<PieceTypeId>,
    /// Condition on the partner, evaluated with the `self_*` and `partner_*` variables.
    #[serde(default)]
    pub condition: Option<ExprString>,
    /// What happens to the partner; it stays untouched when unset.
    #[serde(default)]
    pub action: Option<PartnerAction>,
}

/// Ownership a [`Partner`] must have, relative to the mover.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PartnerOwner {
    /// Same owner as the mover (for an NPC mover: another NPC).
    #[serde(rename = "self")]
    Mover,
    /// Any piece the mover could capture: another player's piece, or an NPC for players.
    Enemy,
    /// A piece no player owns.
    Npc,
    /// Any piece.
    Any,
}

/// What a [`PartneredMove`] does to one partner.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PartnerAction {
    /// Moves the partner to this square, relative to the mover's start.
    MoveTo(IVec2),
    /// Removes the partner, crediting the mover's owner as for a capture.
    Capture,
    /// Turns the partner into another piece type.
    Becomes(PieceTypeId),
}
