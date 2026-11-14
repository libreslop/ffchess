//! Move validation shared by players, NPCs, and client prediction.
//!
//! Validating a move builds a [`MovePlan`]: every change the move makes to the board. Nothing
//! changes while planning, so a rejected move leaves the board untouched and applying a plan
//! needs no further checks.

use super::catalog::Catalog;
use crate::models::{GameState, MoveTiming, Piece, PieceConfig};
use crate::protocol::GameError;
use crate::types::{BoardCoord, ExprVars, MoveCount, PieceId, PieceTypeId, PlayerId};
use glam::IVec2;
use std::collections::HashSet;

/// Whether a move only relocates pieces or also takes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKind {
    Quiet,
    Capture,
}

/// A legal move of one piece, as offered to players and NPCs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalMove {
    /// Where the mover lands; the target of the move action.
    pub target: BoardCoord,
    pub kind: MoveKind,
    /// Squares of the other pieces the move captures, relocates, or changes, before the move.
    pub affected: Vec<BoardCoord>,
}

impl LegalMove {
    /// Returns the squares that select this move and carry its hint.
    ///
    /// A move that affects other pieces (a capture, castling) is shown on and chosen by those
    /// pieces; any other move by its target.
    pub fn handles(&self) -> &[BoardCoord] {
        if self.affected.is_empty() {
            std::slice::from_ref(&self.target)
        } else {
            &self.affected
        }
    }
}

/// One change a move makes to the board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MoveStep {
    /// Removes the piece, crediting the mover's owner.
    Capture(PieceId),
    /// Moves the piece to a square, restarting its cooldown.
    Relocate { piece_id: PieceId, to: BoardCoord },
    /// Turns the piece into another type.
    Retype { piece_id: PieceId, to: PieceTypeId },
}

impl MoveStep {
    /// Returns the piece this step changes.
    pub(crate) fn piece_id(&self) -> PieceId {
        match self {
            Self::Capture(piece_id)
            | Self::Relocate { piece_id, .. }
            | Self::Retype { piece_id, .. } => *piece_id,
        }
    }
}

/// Every change one move makes, built before anything changes.
///
/// All relocations happen at the same instant: a piece may land on a square that another
/// piece of the same plan leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MovePlan {
    /// The piece the player or NPC moved.
    pub(crate) mover: PieceId,
    /// Captures first, then relocations, then type changes.
    pub(crate) steps: Vec<MoveStep>,
}

impl MovePlan {
    /// Returns whether the plan takes any piece.
    pub(crate) fn kind(&self) -> MoveKind {
        if self
            .steps
            .iter()
            .any(|step| matches!(step, MoveStep::Capture(_)))
        {
            MoveKind::Capture
        } else {
            MoveKind::Quiet
        }
    }

    /// Returns the squares of the pieces other than the mover that the plan changes, as they
    /// stand before the move.
    fn affected_squares(&self, state: &GameState) -> Vec<BoardCoord> {
        let mut squares: Vec<BoardCoord> = self
            .steps
            .iter()
            .map(MoveStep::piece_id)
            .filter(|piece_id| *piece_id != self.mover)
            .filter_map(|piece_id| state.pieces.get(&piece_id).map(|piece| piece.position))
            .collect();
        squares.dedup();
        squares
    }

    /// Returns the board as it would be after the plan, without scoring or cooldowns.
    fn outcome(&self, state: &GameState) -> GameState {
        let mut after = state.clone();
        for step in &self.steps {
            match step {
                MoveStep::Capture(piece_id) => {
                    after.pieces.remove(piece_id);
                }
                MoveStep::Relocate { piece_id, to } => {
                    if let Some(piece) = after.pieces.get_mut(piece_id) {
                        piece.position = *to;
                    }
                }
                MoveStep::Retype { piece_id, to } => {
                    if let Some(piece) = after.pieces.get_mut(piece_id) {
                        piece.piece_type = to.clone();
                    }
                }
            }
        }
        after
    }

    /// Returns true when the board can hold the pieces where the plan, made with `timing`,
    /// puts them.
    ///
    /// Every destination must be on the board and no two pieces may land on the same square.
    /// A destination may only be occupied by a piece this plan moves away or captures, unless
    /// the plan is a premove: the occupant may have left by the time it runs.
    pub(crate) fn fits(&self, state: &GameState, timing: MoveTiming) -> bool {
        let leaving: HashSet<PieceId> = self
            .steps
            .iter()
            .filter(|step| !matches!(step, MoveStep::Retype { .. }))
            .map(MoveStep::piece_id)
            .collect();
        let mut destinations = HashSet::new();
        self.steps.iter().all(|step| match step {
            MoveStep::Relocate { to, .. } => {
                state.board_size.contains(*to)
                    && destinations.insert(*to)
                    && (timing == MoveTiming::Premove
                        || state
                            .piece_at(*to)
                            .is_none_or(|occupant| leaving.contains(&occupant.id)))
            }
            MoveStep::Capture(_) | MoveStep::Retype { .. } => true,
        })
    }
}

impl PieceConfig {
    /// Returns the configured paths used for moves of `kind`.
    pub fn paths(&self, kind: MoveKind) -> &[Vec<IVec2>] {
        match kind {
            MoveKind::Quiet => &self.move_paths,
            MoveKind::Capture => &self.capture_paths,
        }
    }

    /// Returns the variables of a destination requirement.
    pub(crate) fn destination_vars(
        mover_moves: MoveCount,
        capture: bool,
        square_targeted: bool,
    ) -> ExprVars {
        ExprVars::new()
            .with("self_moves", mover_moves)
            .with("move_capture", capture)
            .with("square_targeted", square_targeted)
    }

    /// Returns true when `plan` meets every destination requirement of this piece type that
    /// applies to a move made with `timing`.
    ///
    /// Requirements see the board as it would be after the move, so a king stepping back
    /// along the ray of an attacking rook still counts as targeted.
    fn allows_destination(
        &self,
        catalog: &Catalog,
        state: &GameState,
        mover: &Piece,
        plan: &MovePlan,
        timing: MoveTiming,
    ) -> bool {
        let mut requirements = self
            .destination_requirements
            .iter()
            .filter(|requirement| requirement.applies_to(timing))
            .peekable();
        if requirements.peek().is_none() {
            return true;
        }
        let Some((_, _, vars)) = self.destination_context(catalog, state, mover, plan) else {
            return false;
        };
        requirements.all(|requirement| requirement.condition.holds(&vars))
    }

    /// Returns the squares to blame for `plan` breaking destination requirements that apply
    /// to `timing`.
    ///
    /// A requirement that only fails because the landing square is targeted blames the
    /// attackers; any other failing requirement blames the landing square itself.
    fn destination_culprits(
        &self,
        catalog: &Catalog,
        state: &GameState,
        mover: &Piece,
        plan: &MovePlan,
        timing: MoveTiming,
    ) -> Vec<BoardCoord> {
        let Some((after, landed, vars)) = self.destination_context(catalog, state, mover, plan)
        else {
            return Vec::new();
        };
        let untargeted = vars.clone().with("square_targeted", false);
        let mut culprits = Vec::new();
        for requirement in self
            .destination_requirements
            .iter()
            .filter(|requirement| requirement.applies_to(timing))
            .filter(|requirement| !requirement.condition.holds(&vars))
        {
            if requirement.condition.holds(&untargeted) {
                culprits.extend(
                    catalog
                        .attackers(&after, landed, mover.owner_id)
                        .map(|attacker| attacker.position),
                );
            } else {
                culprits.push(landed);
            }
        }
        culprits
    }

    /// Returns the board after `plan`, where the mover lands, and the variables its
    /// destination requirements see, or `None` when the plan removes the mover.
    fn destination_context(
        &self,
        catalog: &Catalog,
        state: &GameState,
        mover: &Piece,
        plan: &MovePlan,
    ) -> Option<(GameState, BoardCoord, ExprVars)> {
        let after = plan.outcome(state);
        let landed = after.pieces.get(&mover.id)?.position;
        let vars = Self::destination_vars(
            mover.moves_made,
            plan.kind() == MoveKind::Capture,
            catalog.is_targeted(&after, landed, mover.owner_id),
        );
        Some((after, landed, vars))
    }

    /// Lists the squares a piece of this type on `start` might move to, before any checks.
    fn candidate_targets(&self, start: BoardCoord) -> Vec<BoardCoord> {
        let path_targets = self
            .move_paths
            .iter()
            .chain(&self.capture_paths)
            .flatten()
            .map(|step| start + *step);
        let special_targets = self
            .special_moves
            .iter()
            .map(|special| special.target_from(start));
        let mut targets: Vec<BoardCoord> = path_targets.chain(special_targets).collect();
        targets.sort_by_key(|pos| (pos.0.x, pos.0.y));
        targets.dedup();
        targets
    }

    /// Checks that one of the `kind` paths from `start` reaches `end` over empty squares.
    fn reaches(
        &self,
        state: &GameState,
        start: BoardCoord,
        end: BoardCoord,
        kind: MoveKind,
    ) -> Result<(), GameError> {
        let displacement = end - start;
        let mut reachable_paths = self
            .paths(kind)
            .iter()
            .filter_map(|path| {
                path.iter()
                    .position(|step| *step == displacement)
                    .map(|i| &path[..i])
            })
            .peekable();
        if reachable_paths.peek().is_none() {
            return Err(GameError::InvalidMove);
        }

        let is_clear = |approach: &[IVec2]| {
            approach
                .iter()
                .all(|step| state.piece_at(start + *step).is_none())
        };
        if reachable_paths.any(is_clear) {
            Ok(())
        } else {
            Err(GameError::PathBlocked)
        }
    }

    /// Returns the first occupied square before `end` on each `kind` path from `start` that
    /// reaches `end`.
    fn blockers(
        &self,
        state: &GameState,
        start: BoardCoord,
        end: BoardCoord,
        kind: MoveKind,
    ) -> Vec<BoardCoord> {
        let displacement = end - start;
        let mut blockers: Vec<BoardCoord> = self
            .paths(kind)
            .iter()
            .filter_map(|path| {
                let approach = &path[..path.iter().position(|step| *step == displacement)?];
                approach
                    .iter()
                    .map(|step| start + *step)
                    .find(|square| state.piece_at(*square).is_some())
            })
            .collect();
        blockers.sort_by_key(|square| (square.0.x, square.0.y));
        blockers.dedup();
        blockers
    }

    /// Returns the squares to blame for a path move of `mover` to `end` failing with `error`.
    ///
    /// A blocked path blames the pieces in the way; any other failure blames `end`.
    fn path_culprits(
        &self,
        state: &GameState,
        mover: &Piece,
        end: BoardCoord,
        error: &GameError,
    ) -> Vec<BoardCoord> {
        if *error != GameError::PathBlocked {
            return vec![end];
        }
        let kind = if state.piece_at(end).is_some() {
            MoveKind::Capture
        } else {
            MoveKind::Quiet
        };
        self.blockers(state, mover.position, end, kind)
    }

    /// Plans a move along `move_paths` or `capture_paths`.
    ///
    /// Quiet moves need an empty target, captures need a target held by a different owner
    /// (NPCs never capture each other), and every square before the target on the matching
    /// path must be empty.
    fn plan_path_move(
        &self,
        state: &GameState,
        mover: &Piece,
        end: BoardCoord,
    ) -> Result<MovePlan, GameError> {
        let start = mover.position;
        if start == end || !state.board_size.contains(end) {
            return Err(GameError::InvalidMove);
        }

        let (kind, captured) = match state.piece_at(end) {
            Some(target) if target.owner_id == mover.owner_id => {
                return Err(GameError::TargetFriendly);
            }
            Some(target) => (MoveKind::Capture, Some(target.id)),
            None => (MoveKind::Quiet, None),
        };

        self.reaches(state, start, end, kind)?;

        let steps = captured
            .map(MoveStep::Capture)
            .into_iter()
            .chain([MoveStep::Relocate {
                piece_id: mover.id,
                to: end,
            }])
            .collect();
        Ok(MovePlan {
            mover: mover.id,
            steps,
        })
    }
}

impl Catalog {
    /// Checks whether `mover` may move to `end` with `timing`.
    ///
    /// Returns the kind of the legal move.
    pub fn check_move(
        &self,
        state: &GameState,
        mover: &Piece,
        end: BoardCoord,
        timing: MoveTiming,
    ) -> Result<MoveKind, GameError> {
        self.plan_move(state, mover, end, timing)
            .map(|plan| plan.kind())
    }

    /// Lists every legal move of `mover` made with `timing`.
    pub fn legal_moves(
        &self,
        state: &GameState,
        mover: &Piece,
        timing: MoveTiming,
    ) -> Vec<LegalMove> {
        let Ok(config) = self.piece(&mover.piece_type) else {
            return Vec::new();
        };
        config
            .candidate_targets(mover.position)
            .into_iter()
            .filter_map(|end| {
                self.plan_move(state, mover, end, timing)
                    .ok()
                    .map(|plan| LegalMove {
                        target: end,
                        kind: plan.kind(),
                        affected: plan.affected_squares(state),
                    })
            })
            .collect()
    }

    /// Plans the move of `mover` to `end`, made with `timing`.
    ///
    /// Path moves take precedence; special moves are tried only when no path reaches `end`.
    /// When neither applies, the path error explains why. The chosen move must then meet the
    /// mover's destination requirements that apply to `timing`.
    pub(crate) fn plan_move(
        &self,
        state: &GameState,
        mover: &Piece,
        end: BoardCoord,
        timing: MoveTiming,
    ) -> Result<MovePlan, GameError> {
        let config = self.piece(&mover.piece_type)?;
        let plan = config.plan_path_move(state, mover, end).or_else(|error| {
            config
                .special_moves
                .iter()
                .find_map(|special| special.plan(self, state, mover, end, timing))
                .ok_or(error)
        })?;
        if config.allows_destination(self, state, mover, &plan, timing) {
            Ok(plan)
        } else {
            Err(GameError::DestinationRestricted)
        }
    }

    /// Returns the squares to blame when `mover` may not move to `end` with `timing`: the
    /// pieces in the way, the friendly piece on `end`, the attackers of a square the move may
    /// not be targeted on, the missing partner of a special move, and so on.
    ///
    /// Follows the same steps as [`Self::plan_move`]. Failures no other square explains blame
    /// `end`; a legal move blames nothing.
    pub fn move_culprits(
        &self,
        state: &GameState,
        mover: &Piece,
        end: BoardCoord,
        timing: MoveTiming,
    ) -> Vec<BoardCoord> {
        let Ok(config) = self.piece(&mover.piece_type) else {
            return vec![end];
        };
        let plan = match config.plan_path_move(state, mover, end) {
            Ok(plan) => plan,
            Err(error) => {
                let special = config
                    .special_moves
                    .iter()
                    .find_map(|special| special.plan(self, state, mover, end, timing));
                match special {
                    Some(plan) => plan,
                    // A special move aiming at `end` explains the failure better than the
                    // missing path does.
                    None => {
                        return config
                            .special_moves
                            .iter()
                            .filter(|_| error == GameError::InvalidMove)
                            .find_map(|special| special.culprits(self, state, mover, end, timing))
                            .unwrap_or_else(|| config.path_culprits(state, mover, end, &error));
                    }
                }
            }
        };
        config.destination_culprits(self, state, mover, &plan, timing)
    }

    /// Returns true when a piece of another owner than `owner` could capture onto `square`.
    ///
    /// Only `capture_paths` count, and only over the current board: cooldowns are ignored and
    /// the square does not need to be occupied. NPCs never target other NPCs' squares.
    pub fn is_targeted(
        &self,
        state: &GameState,
        square: BoardCoord,
        owner: Option<PlayerId>,
    ) -> bool {
        self.attackers(state, square, owner).next().is_some()
    }

    /// Returns the pieces that make `square` targeted for `owner`, as in
    /// [`Self::is_targeted`].
    pub(crate) fn attackers<'a>(
        &'a self,
        state: &'a GameState,
        square: BoardCoord,
        owner: Option<PlayerId>,
    ) -> impl Iterator<Item = &'a Piece> {
        state
            .pieces
            .values()
            .filter(move |attacker| attacker.owner_id != owner && attacker.position != square)
            .filter(move |attacker| {
                self.piece(&attacker.piece_type).is_ok_and(|config| {
                    config
                        .reaches(state, attacker.position, square, MoveKind::Capture)
                        .is_ok()
                })
            })
    }
}
