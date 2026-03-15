use crate::reducer::Pmove;
use common::models::{GameState, Piece};
use common::types::PieceId;
use glam::IVec2;
use std::collections::HashMap;

/// Animation state for a piece transitioning between tiles.
#[derive(Clone)]
pub struct PieceAnim {
    pub start: IVec2,
    pub end: IVec2,
    pub started_at: f64,
}

pub const MOVE_ANIM_MS: f64 = 200.0;

pub fn apply_visible_ghosts(
    ghosts: &mut HashMap<PieceId, Piece>,
    pm_queue: &[Pmove],
    state: &GameState,
) {
    for pm in pm_queue {
        if !pm_visible(pm, state) {
            continue;
        }

        if let Some(p) = ghosts.get_mut(&pm.piece_id) {
            p.position = pm.target;
        }
    }
}

pub fn pm_visible(pm: &Pmove, state: &GameState) -> bool {
    // Show ghosts/paths for any queued move as long as the piece still exists.
    state.pieces.contains_key(&pm.piece_id)
}
