//! Spawning and autonomous movement of NPC pieces.

use super::world::World;
use crate::spawning::find_spawn_pos;
use common::models::{Piece, PieceConfig};
use common::rules::{MoveKind, Simulation};
use common::types::{BoardCoord, PieceId, TimestampMs};
use rand::Rng;
use rand::seq::SliceRandom;

/// NPCs start hunting player pieces closer than this many squares.
const HUNT_RADIUS: f32 = 12.0;

impl World {
    /// Spawns one NPC per configured type that is below its limit.
    pub(super) fn spawn_npcs(&mut self, now: TimestampMs) {
        let setup = self.setup.clone();
        let mut rng = rand::thread_rng();
        for limit in &setup.mode.npc_limits {
            let current = self
                .state
                .pieces
                .values()
                .filter(|piece| !piece.is_player_owned() && piece.piece_type == limit.piece_id)
                .count();
            if current >= limit.max_count(self.state.players.len()) {
                continue;
            }

            let mut piece = Piece::new(
                None,
                limit.piece_id.clone(),
                find_spawn_pos(&self.state, &mut rng),
            );
            piece.cooldown_ms = setup.catalog.cooldown_of(&limit.piece_id);
            // Stagger first moves so freshly spawned NPCs do not all move in lockstep.
            let stagger = rng.gen_range(0..piece.cooldown_ms.as_i64().max(1));
            piece.last_move_time = TimestampMs::from_millis(now.as_i64() - stagger);
            self.state.pieces.insert(piece.id, piece);
        }
    }

    /// Moves every NPC whose cooldown has elapsed.
    pub(super) fn tick_npcs(&mut self, now: TimestampMs) {
        let ready: Vec<PieceId> = self
            .state
            .pieces
            .values()
            .filter(|piece| !piece.is_player_owned() && piece.is_ready(now))
            .map(|piece| piece.id)
            .collect();
        for piece_id in ready {
            self.tick_npc(piece_id, now);
        }
    }

    /// Moves one NPC, if it is still on the board and has a legal move.
    fn tick_npc(&mut self, piece_id: PieceId, now: TimestampMs) {
        let catalog = self.setup.catalog.clone();
        let Some(piece) = self.state.pieces.get(&piece_id) else {
            return;
        };
        let Ok(config) = catalog.piece(&piece.piece_type) else {
            return;
        };
        let Some(target) = self.choose_npc_move(config, piece.position) else {
            return;
        };
        if let Ok(effects) =
            Simulation::new(&catalog, &mut self.state, now).apply_move(piece_id, target)
        {
            self.apply_effects(None, effects);
        }
    }

    /// Chooses the next square for an NPC at `position`.
    ///
    /// NPCs close in on the nearest player piece within the hunt radius, otherwise wander.
    fn choose_npc_move(&self, config: &PieceConfig, position: BoardCoord) -> Option<BoardCoord> {
        let moves = config.legal_moves(&self.state, position, None);
        let prey = self
            .state
            .pieces
            .values()
            .filter(|piece| piece.is_player_owned())
            .min_by_key(|piece| piece.position.distance_squared(position))
            .filter(|piece| (piece.position - position).as_vec2().length() < HUNT_RADIUS);

        match prey {
            Some(prey) => moves
                .into_iter()
                .min_by_key(|(target, _)| target.distance_squared(prey.position))
                .map(|(target, _)| target),
            None => {
                let quiet: Vec<BoardCoord> = moves
                    .into_iter()
                    .filter(|(_, kind)| *kind == MoveKind::Quiet)
                    .map(|(target, _)| target)
                    .collect();
                quiet.choose(&mut rand::thread_rng()).copied()
            }
        }
    }
}
