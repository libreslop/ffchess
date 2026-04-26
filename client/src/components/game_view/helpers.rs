//! Helper utilities for the game view component.

use crate::reducer::Pmove;
use common::models::{GameState, Piece};
use common::types::{BoardCoord, PieceId};
use std::collections::HashMap;

/// Duration of a single move animation in milliseconds.
pub const MOVE_ANIM_MS: f64 = 200.0;

/// Applies visible queued moves to ghost piece positions.
///
/// `ghosts` is the mutable ghost map, `pm_queue` is the pending move list,
/// and `state` is the current game state. Returns nothing.
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
            p.position = BoardCoord(pm.target);
        }
    }
}

/// Determines if a pending move is visible based on current game state.
///
/// `pm` is the pending move and `state` is the game state. Returns `true` if visible.
pub fn pm_visible(pm: &Pmove, state: &GameState) -> bool {
    // Show ghosts/paths for any queued move as long as the piece still exists.
    state.pieces.contains_key(&pm.piece_id)
}
