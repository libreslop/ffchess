//! Speed lines left behind by pieces that jump between squares when snapshots move them.

use crate::canvas::MoveBlur;
use common::models::Piece;
use common::types::{BoardCoord, PieceId};
use std::collections::HashMap;

/// Duration of the speed lines behind a moved piece, in milliseconds.
const BLUR_MS: f64 = 300.0;
/// Number of distinct frames the speed lines step through, for a choppy pixel-art feel.
const BLUR_FRAMES: f64 = 4.0;

/// One piece's move, blurring since `started_at`.
struct ActiveBlur {
    from: BoardCoord,
    to: BoardCoord,
    started_at: f64,
}

impl ActiveBlur {
    /// Returns the blur at `now`, its progress stepped to whole frames, or `None` once
    /// finished.
    fn at(&self, piece_id: PieceId, now: f64) -> Option<MoveBlur> {
        let frame = ((now - self.started_at) / BLUR_MS * BLUR_FRAMES).floor();
        (frame < BLUR_FRAMES).then(|| MoveBlur {
            piece_id,
            from: self.from,
            to: self.to,
            progress: frame.max(0.0) / BLUR_FRAMES,
        })
    }
}

/// Tracks the last known square of every piece and blurs the moves between them.
#[derive(Default)]
pub(super) struct MoveBlurs {
    last_positions: HashMap<PieceId, BoardCoord>,
    active: HashMap<PieceId, ActiveBlur>,
}

impl MoveBlurs {
    /// Starts blurs for pieces whose square changed since the previous snapshot.
    pub(super) fn observe(&mut self, pieces: &HashMap<PieceId, Piece>, now: f64) {
        self.active.retain(|id, _| pieces.contains_key(id));
        for (id, piece) in pieces {
            if let Some(previous) = self.last_positions.get(id)
                && *previous != piece.position
            {
                self.active.insert(
                    *id,
                    ActiveBlur {
                        from: *previous,
                        to: piece.position,
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

    /// Returns the blurs showing at `now`, dropping finished ones.
    pub(super) fn blurs_at(&mut self, now: f64) -> Vec<MoveBlur> {
        let mut blurs = Vec::new();
        self.active.retain(|id, blur| match blur.at(*id, now) {
            Some(shown) => {
                blurs.push(shown);
                true
            }
            None => false,
        });
        blurs
    }
}
