//! Smooth sliding of pieces between squares when snapshots move them.

use crate::math::{Vec2, vec2};
use common::models::Piece;
use common::types::{BoardCoord, PieceId};
use std::collections::HashMap;

/// Duration of a single move animation in milliseconds.
const MOVE_ANIM_MS: f64 = 200.0;

/// One piece sliding from `start` to `end`.
struct PieceAnim {
    start: BoardCoord,
    end: BoardCoord,
    started_at: f64,
}

impl PieceAnim {
    /// Returns the animated square position at `now`, or `None` once finished.
    fn position_at(&self, now: f64) -> Option<Vec2> {
        let progress = ((now - self.started_at) / MOVE_ANIM_MS).clamp(0.0, 1.0);
        (progress < 1.0).then(|| {
            let start = self.start.0.as_dvec2();
            let end = self.end.0.as_dvec2();
            start + (end - start) * progress
        })
    }
}

/// Tracks the last known square of every piece and animates changes.
#[derive(Default)]
pub(super) struct PieceAnimations {
    last_positions: HashMap<PieceId, BoardCoord>,
    active: HashMap<PieceId, PieceAnim>,
}

impl PieceAnimations {
    /// Starts animations for pieces whose square changed since the previous snapshot.
    pub(super) fn observe(&mut self, pieces: &HashMap<PieceId, Piece>, now: f64) {
        self.active.retain(|id, _| pieces.contains_key(id));
        for (id, piece) in pieces {
            if let Some(previous) = self.last_positions.get(id)
                && *previous != piece.position
            {
                self.active.insert(
                    *id,
                    PieceAnim {
                        start: *previous,
                        end: piece.position,
                        started_at: now,
                    },
                );
            }
        }
        self.last_positions = pieces
            .iter()
            .map(|(id, piece)| (*id, piece.position))
            .collect();
    }

    /// Returns the in-between positions of animating pieces, dropping finished animations.
    pub(super) fn positions_at(&mut self, now: f64) -> HashMap<PieceId, Vec2> {
        let mut positions = HashMap::new();
        self.active.retain(|id, anim| match anim.position_at(now) {
            Some(pos) => {
                positions.insert(*id, vec2(pos.x, pos.y));
                true
            }
            None => false,
        });
        positions
    }
}
