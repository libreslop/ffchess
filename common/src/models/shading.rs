//! Config of piece shading: visual cues drawn on pieces while a condition holds.

use crate::types::{ColorHex, ExprString, OpacityPercent};
use serde::{Deserialize, Serialize};

/// A visual cue drawn on a piece while its condition holds, seen only by chosen viewers.
///
/// Both conditions are evaluated on the authoritative board of the viewer, with the threat
/// relations of every piece (which pieces could capture which).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShadingRule {
    /// Condition on the piece, evaluated with the `self_*` variables; always holds when unset.
    #[serde(default)]
    pub condition: Option<ExprString>,
    /// Condition on the viewer, evaluated with the `self_*` and `viewer_*` variables; every
    /// viewer sees the shading when unset.
    #[serde(default)]
    pub visible_to: Option<ExprString>,
    /// What is drawn.
    pub effects: Vec<ShadingEffect>,
}

/// One thing a [`ShadingRule`] draws.
///
/// The `type` field of the config selects the variant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShadingEffect {
    /// Fills the piece's square behind the piece.
    Background {
        color: ShadingColor,
        #[serde(default = "OpacityPercent::translucent")]
        opacity: OpacityPercent,
    },
    /// Draws a short mark on the rim of the piece's square, pointing at each related piece.
    LineTowards {
        /// Which pieces the marks point at.
        towards: PieceRelation,
        /// Condition on each related piece, evaluated with the `self_*`, `viewer_*`, and
        /// `other_*` variables; every related piece gets a mark when unset.
        #[serde(default)]
        filter: Option<ExprString>,
        color: ShadingColor,
        #[serde(default = "OpacityPercent::opaque")]
        opacity: OpacityPercent,
    },
}

/// Pieces related to a shaded piece through capture threats.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PieceRelation {
    /// Pieces that could capture the shaded piece: the sources of its threats.
    Attackers,
    /// Pieces the shaded piece could capture.
    Targets,
}

/// Color of a shading effect.
///
/// Written as `"owner"`, `"viewer"`, `"other"`, `{ "most_common": "attackers" }`, or a literal
/// `"#rrggbb"` color.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShadingColor {
    /// Team color of the shaded piece's owner.
    Owner,
    /// Team color of the viewer.
    Viewer,
    /// Team color of the related piece's owner; only meaningful for `line_towards`, and the
    /// owner's color elsewhere.
    Other,
    /// Team color most common among the pieces in this relation to the shaded piece (the mode
    /// of their owners); the owner's color when there are none. Ties go to the same team on
    /// every board.
    MostCommon(PieceRelation),
    /// A fixed color.
    #[serde(untagged)]
    Fixed(ColorHex),
}
