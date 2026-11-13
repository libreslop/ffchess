//! Path-based move validation shared by players, NPCs, and client prediction.

use crate::models::{GameState, PieceConfig};
use crate::protocol::GameError;
use crate::types::{BoardCoord, PlayerId};
use glam::IVec2;

/// Whether a move lands on an empty square or takes an enemy piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKind {
    Quiet,
    Capture,
}

impl PieceConfig {
    /// Returns the configured paths used for moves of `kind`.
    pub fn paths(&self, kind: MoveKind) -> &[Vec<IVec2>] {
        match kind {
            MoveKind::Quiet => &self.move_paths,
            MoveKind::Capture => &self.capture_paths,
        }
    }

    /// Checks whether a piece of this type owned by `owner` may move from `start` to `end`.
    ///
    /// Quiet moves need an empty target, captures need a target held by a different owner
    /// (NPCs never capture each other), and every square before the target on the matching
    /// path must be empty. Returns the kind of the legal move.
    pub fn check_move(
        &self,
        state: &GameState,
        start: BoardCoord,
        end: BoardCoord,
        owner: Option<PlayerId>,
    ) -> Result<MoveKind, GameError> {
        if start == end || !state.board_size.contains(end) {
            return Err(GameError::InvalidMove);
        }

        let kind = match state.piece_at(end) {
            Some(target) if target.owner_id == owner => return Err(GameError::TargetFriendly),
            Some(_) => MoveKind::Capture,
            None => MoveKind::Quiet,
        };

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
            Ok(kind)
        } else {
            Err(GameError::PathBlocked)
        }
    }

    /// Lists every legal destination for a piece of this type at `start`.
    pub fn legal_moves(
        &self,
        state: &GameState,
        start: BoardCoord,
        owner: Option<PlayerId>,
    ) -> Vec<(BoardCoord, MoveKind)> {
        let mut targets: Vec<BoardCoord> = self
            .move_paths
            .iter()
            .chain(&self.capture_paths)
            .flatten()
            .map(|step| start + *step)
            .collect();
        targets.sort_by_key(|pos| (pos.0.x, pos.0.y));
        targets.dedup();
        targets
            .into_iter()
            .filter_map(|end| {
                self.check_move(state, start, end, owner)
                    .ok()
                    .map(|kind| (end, kind))
            })
            .collect()
    }
}
