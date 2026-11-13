//! Resolution of shading rules into the marks one viewer sees on the board.

use super::catalog::Catalog;
use super::threats::ThreatMap;
use crate::models::{GameState, Piece, PieceRelation, ShadingColor, ShadingEffect, ShadingRule};
use crate::types::{ColorHex, ExprString, ExprVars, MoveCount, OpacityPercent, PieceId, PlayerId};
use std::collections::HashMap;

/// Color a mark is drawn in, resolved for one piece and viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaintColor {
    /// Team color of a player, or the NPC color for `None` and unknown players.
    Team(Option<PlayerId>),
    /// A fixed color.
    Fixed(ColorHex),
}

impl PaintColor {
    /// Returns the hex color of this paint, looking team colors up in `state`.
    ///
    /// Returns `None` for NPCs and players no longer in the game.
    pub fn hex<'a>(&'a self, state: &'a GameState) -> Option<&'a ColorHex> {
        match self {
            Self::Team(owner) => owner
                .and_then(|owner| state.players.get(&owner))
                .map(|player| &player.color),
            Self::Fixed(color) => Some(color),
        }
    }
}

/// One mark drawn on a piece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadeMark {
    /// Fills the piece's square behind the piece.
    Background {
        color: PaintColor,
        opacity: OpacityPercent,
    },
    /// A short mark on the rim of the piece's square, pointing at another piece.
    Line {
        towards: PieceId,
        color: PaintColor,
        opacity: OpacityPercent,
    },
}

/// A mark on one piece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shade {
    pub piece_id: PieceId,
    pub mark: ShadeMark,
}

/// The board, its threats, and the viewer that shading rules are resolved against.
struct ShadingScene<'a> {
    state: &'a GameState,
    threats: ThreatMap,
    viewer: PlayerId,
}

impl ShadingScene<'_> {
    /// Returns true when `piece` belongs to the viewer.
    fn is_viewers(&self, piece: &Piece) -> bool {
        piece.owner_id == Some(self.viewer)
    }

    /// Returns the owner most common among the pieces in `relation` to `piece`, or `None`
    /// when no piece stands in that relation.
    ///
    /// Ties are broken by player id so the color does not flicker between boards.
    fn most_common_owner(
        &self,
        piece: &Piece,
        relation: PieceRelation,
    ) -> Option<Option<PlayerId>> {
        let mut counts: HashMap<Option<PlayerId>, usize> = HashMap::new();
        for owner in self
            .threats
            .related(piece.id, relation)
            .iter()
            .filter_map(|id| self.state.pieces.get(id))
            .map(|related| related.owner_id)
        {
            *counts.entry(owner).or_default() += 1;
        }
        counts
            .into_iter()
            .max_by_key(|(owner, count)| (*count, owner.map(|owner| owner.0)))
            .map(|(owner, _)| owner)
    }

    /// Counts the pieces in `ids` that belong to the viewer and match `predicate`.
    fn count_viewers(&self, ids: &[PieceId], predicate: impl Fn(&Piece) -> bool) -> usize {
        ids.iter()
            .filter_map(|id| self.state.pieces.get(id))
            .filter(|piece| self.is_viewers(piece) && predicate(piece))
            .count()
    }
}

/// What a shading condition knows about the shaded piece: the `self_*` variables.
#[derive(Default)]
struct PieceFacts {
    is_king: bool,
    is_npc: bool,
    moves: MoveCount,
    attackers: usize,
    targets: usize,
    targets_king: bool,
}

impl PieceFacts {
    /// Gathers the facts of `piece` in `scene`.
    fn of(piece: &Piece, scene: &ShadingScene<'_>) -> Self {
        let targets = scene.threats.targets_of(piece.id);
        Self {
            is_king: piece.piece_type.is_king(),
            is_npc: !piece.is_player_owned(),
            moves: piece.moves_made,
            attackers: scene.threats.attackers_of(piece.id).len(),
            targets: targets.len(),
            targets_king: targets
                .iter()
                .filter_map(|id| scene.state.pieces.get(id))
                .any(|target| target.piece_type.is_king()),
        }
    }

    /// Returns the `self_*` variables.
    fn vars(&self) -> ExprVars {
        ExprVars::new()
            .with("self_is_king", self.is_king)
            .with("self_is_npc", self.is_npc)
            .with("self_moves", self.moves)
            .with("self_targeted", self.attackers > 0)
            .with("self_attackers", self.attackers)
            .with("self_targets", self.targets)
            .with("self_targets_king", self.targets_king)
    }
}

/// What a visibility condition knows about the viewer: the `viewer_*` variables.
#[derive(Default)]
struct ViewerFacts {
    is_player: bool,
    is_owner: bool,
    /// Viewer pieces that could capture the shaded piece.
    attacking_self: usize,
    /// Viewer pieces the shaded piece could capture.
    targeted_by_self: usize,
    king_targeted_by_self: bool,
}

impl ViewerFacts {
    /// Gathers the viewer's facts about `piece` in `scene`.
    fn of(piece: &Piece, scene: &ShadingScene<'_>) -> Self {
        let targets = scene.threats.targets_of(piece.id);
        Self {
            is_player: scene.state.players.contains_key(&scene.viewer),
            is_owner: scene.is_viewers(piece),
            attacking_self: scene.count_viewers(scene.threats.attackers_of(piece.id), |_| true),
            targeted_by_self: scene.count_viewers(targets, |_| true),
            king_targeted_by_self: scene
                .count_viewers(targets, |target| target.piece_type.is_king())
                > 0,
        }
    }

    /// Adds the `viewer_*` variables to `vars`.
    fn extend(&self, vars: ExprVars) -> ExprVars {
        vars.with("viewer_is_player", self.is_player)
            .with("viewer_is_owner", self.is_owner)
            .with("viewer_is_opponent", self.is_player && !self.is_owner)
            .with("viewer_attackers", self.attacking_self)
            .with("viewer_targeted", self.targeted_by_self)
            .with("viewer_king_targeted", self.king_targeted_by_self)
    }
}

/// What a `line_towards` filter knows about the related piece: the `other_*` variables.
#[derive(Default)]
struct OtherFacts {
    is_king: bool,
    is_npc: bool,
    is_viewers: bool,
    moves: MoveCount,
}

impl OtherFacts {
    /// Gathers the facts of the related piece `other` in `scene`.
    fn of(other: &Piece, scene: &ShadingScene<'_>) -> Self {
        Self {
            is_king: other.piece_type.is_king(),
            is_npc: !other.is_player_owned(),
            is_viewers: scene.is_viewers(other),
            moves: other.moves_made,
        }
    }

    /// Adds the `other_*` variables to `vars`.
    fn extend(&self, vars: ExprVars) -> ExprVars {
        vars.with("other_is_king", self.is_king)
            .with("other_is_npc", self.is_npc)
            .with("other_is_viewers", self.is_viewers)
            .with("other_moves", self.moves)
    }
}

impl Catalog {
    /// Resolves every shading rule on `state` into the marks `viewer` sees.
    ///
    /// Threats are only worked out when some piece type has shading rules.
    pub fn shading(&self, state: &GameState, viewer: PlayerId) -> Vec<Shade> {
        if self.pieces.values().all(|config| config.shading.is_empty()) {
            return Vec::new();
        }
        let scene = ShadingScene {
            state,
            threats: self.threats(state),
            viewer,
        };
        state
            .pieces
            .values()
            .filter_map(|piece| {
                self.pieces
                    .get(&piece.piece_type)
                    .map(|config| (piece, config))
            })
            .flat_map(|(piece, config)| {
                config
                    .shading
                    .iter()
                    .flat_map(|rule| rule.resolve(piece, &scene))
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

impl ShadingRule {
    /// Checks that every condition of the rule is valid.
    pub(crate) fn validate(&self) -> Result<(), String> {
        let piece_sample = PieceFacts::default().vars();
        let viewer_sample = ViewerFacts::default().extend(piece_sample.clone());
        let other_sample = OtherFacts::default().extend(viewer_sample.clone());
        let filters = self.effects.iter().filter_map(|effect| match effect {
            ShadingEffect::LineTowards { filter, .. } => filter.as_ref(),
            ShadingEffect::Background { .. } => None,
        });
        self.condition
            .iter()
            .map(|condition| (condition, &piece_sample))
            .chain(
                self.visible_to
                    .iter()
                    .map(|visible| (visible, &viewer_sample)),
            )
            .chain(filters.map(|filter| (filter, &other_sample)))
            .try_for_each(|(condition, sample)| condition.check_condition(sample))
    }

    /// Returns the marks this rule puts on `piece`, or none when its conditions fail.
    fn resolve(&self, piece: &Piece, scene: &ShadingScene<'_>) -> Vec<Shade> {
        let piece_vars = PieceFacts::of(piece, scene).vars();
        if !ExprString::holds_if_set(self.condition.as_ref(), &piece_vars) {
            return Vec::new();
        }
        let viewer_vars = ViewerFacts::of(piece, scene).extend(piece_vars);
        if !ExprString::holds_if_set(self.visible_to.as_ref(), &viewer_vars) {
            return Vec::new();
        }
        self.effects
            .iter()
            .flat_map(|effect| effect.resolve(piece, scene, &viewer_vars))
            .map(|mark| Shade {
                piece_id: piece.id,
                mark,
            })
            .collect()
    }
}

impl ShadingEffect {
    /// Returns the marks this effect puts on `piece`.
    ///
    /// `vars` holds the `self_*` and `viewer_*` variables.
    fn resolve(&self, piece: &Piece, scene: &ShadingScene<'_>, vars: &ExprVars) -> Vec<ShadeMark> {
        match self {
            Self::Background { color, opacity } => vec![ShadeMark::Background {
                color: color.resolve(piece, None, scene),
                opacity: *opacity,
            }],
            Self::LineTowards {
                towards,
                filter,
                color,
                opacity,
            } => scene
                .threats
                .related(piece.id, *towards)
                .iter()
                .filter_map(|id| scene.state.pieces.get(id))
                .filter(|other| {
                    let vars = OtherFacts::of(other, scene).extend(vars.clone());
                    ExprString::holds_if_set(filter.as_ref(), &vars)
                })
                .map(|other| ShadeMark::Line {
                    towards: other.id,
                    color: color.resolve(piece, Some(other), scene),
                    opacity: *opacity,
                })
                .collect(),
        }
    }
}

impl ShadingColor {
    /// Resolves the color for `piece` and the related piece `other`, if any, in `scene`.
    fn resolve(
        &self,
        piece: &Piece,
        other: Option<&Piece>,
        scene: &ShadingScene<'_>,
    ) -> PaintColor {
        match self {
            Self::Owner => PaintColor::Team(piece.owner_id),
            Self::Viewer => PaintColor::Team(Some(scene.viewer)),
            Self::Other => PaintColor::Team(other.unwrap_or(piece).owner_id),
            Self::MostCommon(relation) => PaintColor::Team(
                scene
                    .most_common_owner(piece, *relation)
                    .unwrap_or(piece.owner_id),
            ),
            Self::Fixed(color) => PaintColor::Fixed(color.clone()),
        }
    }
}
