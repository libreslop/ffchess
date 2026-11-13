//! Capture threats between pieces: which pieces could capture which on one board.

use super::catalog::Catalog;
use crate::models::{GameState, Piece, PieceRelation};
use crate::types::{BoardCoord, PieceId};
use std::collections::{HashMap, HashSet};

/// Who could capture whom on one board.
///
/// A piece threatens another when the other has a different owner and lies on one of its
/// `capture_paths` with every earlier square of that path empty, the same rule as
/// [`Catalog::is_targeted`]. Cooldowns are ignored and NPCs never threaten each other.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThreatMap {
    attackers: HashMap<PieceId, Vec<PieceId>>,
    targets: HashMap<PieceId, Vec<PieceId>>,
}

impl ThreatMap {
    /// Returns the pieces that could capture `piece`.
    pub fn attackers_of(&self, piece: PieceId) -> &[PieceId] {
        self.attackers.get(&piece).map_or(&[], Vec::as_slice)
    }

    /// Returns the pieces `piece` could capture.
    pub fn targets_of(&self, piece: PieceId) -> &[PieceId] {
        self.targets.get(&piece).map_or(&[], Vec::as_slice)
    }

    /// Returns the pieces standing in `relation` to `piece`.
    pub fn related(&self, piece: PieceId, relation: PieceRelation) -> &[PieceId] {
        match relation {
            PieceRelation::Attackers => self.attackers_of(piece),
            PieceRelation::Targets => self.targets_of(piece),
        }
    }

    /// Records that `attacker` could capture `victim`.
    fn insert(&mut self, attacker: PieceId, victim: PieceId) {
        self.attackers.entry(victim).or_default().push(attacker);
        self.targets.entry(attacker).or_default().push(victim);
    }
}

impl Catalog {
    /// Works out which pieces could capture which on `state`.
    ///
    /// Each capture path is walked once up to its first occupied square, so the cost grows
    /// with the total path length rather than with every pair of pieces.
    pub fn threats(&self, state: &GameState) -> ThreatMap {
        let occupants: HashMap<BoardCoord, &Piece> = state
            .pieces
            .values()
            .map(|piece| (piece.position, piece))
            .collect();
        let mut threats = ThreatMap::default();
        for attacker in state.pieces.values() {
            let Ok(config) = self.piece(&attacker.piece_type) else {
                continue;
            };
            // A set, since several paths may reach the same victim.
            let victims: HashSet<PieceId> = config
                .capture_paths
                .iter()
                .filter_map(|path| {
                    path.iter()
                        .find_map(|step| occupants.get(&(attacker.position + *step)))
                })
                .filter(|victim| victim.owner_id != attacker.owner_id)
                .map(|victim| victim.id)
                .collect();
            for victim in victims {
                threats.insert(attacker.id, victim);
            }
        }
        threats
    }
}
