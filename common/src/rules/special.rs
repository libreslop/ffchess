//! Planning of special moves such as castling.

use super::catalog::Catalog;
use super::movement::{MovePlan, MoveStep};
use crate::models::{
    GameState, MoveTiming, Partner, PartnerAction, PartnerOwner, PartneredMove, Piece, SpecialMove,
    SquareRequirement,
};
use crate::types::{BoardCoord, ExprString, ExprVars, MoveCount};
use std::collections::HashSet;

impl SpecialMove {
    /// Returns the square this special move sends a mover standing on `start` to.
    pub(crate) fn target_from(&self, start: BoardCoord) -> BoardCoord {
        match self {
            Self::PartneredMove(partnered) => start + partnered.target,
        }
    }

    /// Checks that every condition is valid and every piece type named exists in `catalog`.
    pub(crate) fn validate(&self, catalog: &Catalog) -> Result<(), String> {
        match self {
            Self::PartneredMove(partnered) => partnered.validate(catalog),
        }
    }

    /// Plans this special move of `mover` onto `end`, made with `timing`, or returns `None`
    /// when it is not legal.
    pub(crate) fn plan(
        &self,
        catalog: &Catalog,
        state: &GameState,
        mover: &Piece,
        end: BoardCoord,
        timing: MoveTiming,
    ) -> Option<MovePlan> {
        match self {
            Self::PartneredMove(partnered) => partnered.plan(catalog, state, mover, end, timing),
        }
    }
}

impl PartneredMove {
    /// Checks the conditions and piece types of the move, its squares, and its partners.
    fn validate(&self, catalog: &Catalog) -> Result<(), String> {
        let mover_sample = Self::vars(MoveCount::default(), false);
        let partner_sample = Partner::vars(&mover_sample, MoveCount::default(), false);
        let square_sample = SquareRequirement::vars(&mover_sample, true, false);
        let conditions = self
            .condition
            .iter()
            .map(|condition| (condition, &mover_sample))
            .chain(
                self.requirements
                    .iter()
                    .map(|requirement| (&requirement.condition, &square_sample)),
            )
            .chain(self.partners.iter().filter_map(|partner| {
                partner
                    .condition
                    .as_ref()
                    .map(|condition| (condition, &partner_sample))
            }));
        for (condition, sample) in conditions {
            condition.check_condition(sample)?;
        }

        let partner_types = self.partners.iter().flat_map(|partner| {
            let action_type = match &partner.action {
                Some(PartnerAction::Becomes(piece_type)) => Some(piece_type),
                _ => None,
            };
            partner.types.iter().chain(action_type)
        });
        match partner_types
            .chain(&self.becomes)
            .find(|piece_type| catalog.piece(piece_type).is_err())
        {
            Some(unknown) => Err(format!("unknown piece type `{unknown}`")),
            None => Ok(()),
        }
    }

    /// Returns the variables of the mover's condition.
    fn vars(mover_moves: MoveCount, mover_targeted: bool) -> ExprVars {
        ExprVars::new()
            .with("self_moves", mover_moves)
            .with("self_targeted", mover_targeted)
    }

    /// Plans the move when its target is `end` and every requirement that applies to
    /// `timing` is met.
    fn plan(
        &self,
        catalog: &Catalog,
        state: &GameState,
        mover: &Piece,
        end: BoardCoord,
        timing: MoveTiming,
    ) -> Option<MovePlan> {
        let start = mover.position;
        if start + self.target != end {
            return None;
        }
        let vars = Self::vars(
            mover.moves_made,
            catalog.is_targeted(state, start, mover.owner_id),
        );
        if !ExprString::holds_if_set(self.condition.as_ref(), &vars)
            || !self
                .requirements
                .iter()
                .filter(|requirement| requirement.applies_to(timing))
                .all(|requirement| requirement.holds(catalog, state, mover, &vars))
        {
            return None;
        }

        let partners = self
            .partners
            .iter()
            .map(|partner| {
                partner
                    .find(catalog, state, mover, &vars)
                    .map(|piece| (partner, piece))
            })
            .collect::<Option<Vec<_>>>()?;
        let mut seen = HashSet::from([mover.id]);
        if !partners.iter().all(|(_, piece)| seen.insert(piece.id)) {
            return None;
        }

        let plan = MovePlan {
            mover: mover.id,
            steps: self.steps(mover, end, &partners),
        };
        plan.fits(state, timing).then_some(plan)
    }

    /// Builds the plan steps: captures, then relocations, then type changes.
    fn steps(
        &self,
        mover: &Piece,
        end: BoardCoord,
        partners: &[(&Partner, &Piece)],
    ) -> Vec<MoveStep> {
        let start = mover.position;
        let partner_step = |(partner, piece): &(&Partner, &Piece)| {
            partner.action.as_ref().map(|action| match action {
                PartnerAction::Capture => MoveStep::Capture(piece.id),
                PartnerAction::MoveTo(offset) => MoveStep::Relocate {
                    piece_id: piece.id,
                    to: start + *offset,
                },
                PartnerAction::Becomes(piece_type) => MoveStep::Retype {
                    piece_id: piece.id,
                    to: piece_type.clone(),
                },
            })
        };
        let mover_steps = [
            Some(MoveStep::Relocate {
                piece_id: mover.id,
                to: end,
            }),
            self.becomes.clone().map(|piece_type| MoveStep::Retype {
                piece_id: mover.id,
                to: piece_type,
            }),
        ];
        let mut steps: Vec<MoveStep> = partners
            .iter()
            .filter_map(partner_step)
            .chain(mover_steps.into_iter().flatten())
            .collect();
        // The sort is stable, so steps of one kind keep their listed order.
        steps.sort_by_key(|step| match step {
            MoveStep::Capture(_) => 0,
            MoveStep::Relocate { .. } => 1,
            MoveStep::Retype { .. } => 2,
        });
        steps
    }
}

impl SquareRequirement {
    /// Returns the variables of a square condition: the mover's plus the square's own.
    fn vars(mover_vars: &ExprVars, square_empty: bool, square_targeted: bool) -> ExprVars {
        mover_vars
            .clone()
            .with("square_empty", square_empty)
            .with("square_targeted", square_targeted)
    }

    /// Returns true when the square holds the condition for `mover`.
    ///
    /// The square counts as targeted when an enemy of the mover could capture onto it.
    fn holds(
        &self,
        catalog: &Catalog,
        state: &GameState,
        mover: &Piece,
        mover_vars: &ExprVars,
    ) -> bool {
        let square = mover.position + self.at;
        let vars = Self::vars(
            mover_vars,
            state.piece_at(square).is_none(),
            catalog.is_targeted(state, square, mover.owner_id),
        );
        self.condition.holds(&vars)
    }
}

impl Partner {
    /// Returns the variables of a partner's condition: the mover's plus the partner's own.
    fn vars(mover_vars: &ExprVars, partner_moves: MoveCount, partner_targeted: bool) -> ExprVars {
        mover_vars
            .clone()
            .with("partner_moves", partner_moves)
            .with("partner_targeted", partner_targeted)
    }

    /// Returns the piece filling this partner role for `mover`, if one qualifies.
    ///
    /// `mover_vars` holds the mover's variables; the partner's own are added to them.
    fn find<'a>(
        &self,
        catalog: &Catalog,
        state: &'a GameState,
        mover: &Piece,
        mover_vars: &ExprVars,
    ) -> Option<&'a Piece> {
        let piece = state.piece_at(mover.position + self.at)?;
        let owner_matches = match self.owner {
            PartnerOwner::Mover => piece.owner_id == mover.owner_id,
            PartnerOwner::Enemy => piece.owner_id != mover.owner_id,
            PartnerOwner::Npc => piece.owner_id.is_none(),
            PartnerOwner::Any => true,
        };
        let type_matches = self.types.is_empty() || self.types.contains(&piece.piece_type);
        if !owner_matches || !type_matches {
            return None;
        }
        let vars = Self::vars(
            mover_vars,
            piece.moves_made,
            catalog.is_targeted(state, piece.position, piece.owner_id),
        );
        ExprString::holds_if_set(self.condition.as_ref(), &vars).then_some(piece)
    }
}
