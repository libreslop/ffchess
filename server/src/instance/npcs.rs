//! Automated movement logic for NPC pieces.

use super::GameInstance;
use crate::time::now_ms;
use common::logic::MoveValidationParams;
use common::models::{GameState, Piece, PieceConfig};
use common::types::{BoardSize, DurationMs, PieceId, PieceTypeId, TimestampMs, BoardCoord};
use glam::IVec2;
use rand::Rng;

#[derive(Clone, Copy)]
struct NpcMove {
    target: IVec2,
    is_capture: bool,
}

impl GameInstance {
    /// Spawns new NPCs up to the limits defined in the mode configuration.
    ///
    /// Checks current piece counts and uses `find_spawn_pos` to place new NPCs.
    pub async fn spawn_npcs(&self) {
        let now = now_ms();
        let mut game = self.game.write().await;
        for limit in &self.mode_config.npc_limits {
            let current_count = game
                .pieces
                .values()
                .filter(|p| p.owner_id.is_none() && p.piece_type == limit.piece_id)
                .count();

            let mut vars = std::collections::HashMap::new();
            vars.insert("player_count".to_string(), game.players.len() as f64);
            let max_npcs = common::logic::evaluate_expression(&limit.max_expr, &vars) as usize;

            if current_count >= max_npcs {
                continue;
            }

            let spawn_pos = crate::spawning::find_spawn_pos(&game);
            let piece_id = PieceId::new();
            let cooldown_ms = self
                .piece_configs
                .get(&limit.piece_id)
                .map(|config| config.cooldown_ms)
                .unwrap_or_else(|| DurationMs::from_millis(2000));
            game.pieces.insert(
                piece_id,
                Piece {
                    id: piece_id,
                    owner_id: None,
                    piece_type: limit.piece_id.clone(),
                    position: BoardCoord(spawn_pos),
                    last_move_time: self.initial_npc_last_move(now, cooldown_ms),
                    cooldown_ms,
                },
            );
        }
    }

    /// Advances all NPCs if they can move this tick.
    pub async fn tick_npcs(&self) {
        let now = now_ms();
        let mut game = self.game.write().await;
        let board_size = game.board_size;
        let piece_ids: Vec<_> = game.pieces.keys().copied().collect();
        for piece_id in piece_ids {
            let is_npc = game.pieces.get(&piece_id).map(|p| p.owner_id.is_none()).unwrap_or(false);
            if is_npc {
                self.tick_npc(piece_id, now, board_size, &mut game).await;
            }
        }
    }

    /// Advances one NPC if it can move this tick.
    pub(super) async fn tick_npc(
        &self,
        piece_id: PieceId,
        now: TimestampMs,
        board_size: BoardSize,
        game: &mut GameState,
    ) {
        let Some((piece_type, position, last_move_time, cooldown_ms)) =
            self.npc_motion_state(piece_id, game)
        else {
            return;
        };

        if now - last_move_time < cooldown_ms {
            return;
        }

        let Some(piece_config) = self.piece_configs.get(&piece_type) else {
            return;
        };

        let target = self.choose_npc_move(piece_config, position, board_size, game);
        if let Some(npc_move) = target {
            self.update_piece_motion(
                piece_id,
                npc_move.target,
                now,
                piece_config,
                game,
            );
        }
    }

    fn npc_motion_state(
        &self,
        piece_id: PieceId,
        game: &GameState,
    ) -> Option<(PieceTypeId, IVec2, TimestampMs, DurationMs)> {
        let piece = game.pieces.get(&piece_id)?;
        Some((
            piece.piece_type.clone(),
            piece.position.into(),
            piece.last_move_time,
            piece.cooldown_ms,
        ))
    }

    /// Chooses the next move for an NPC, preferring nearby captures and chases.
    fn choose_npc_move(
        &self,
        piece_config: &PieceConfig,
        position: IVec2,
        board_size: BoardSize,
        game: &GameState,
    ) -> Option<NpcMove> {
        let nearest_player_piece = game
            .pieces
            .values()
            .filter(|piece| piece.owner_id.is_some())
            .min_by_key(|piece| (piece.position - position).as_vec2().length_squared() as i32);

        if let Some(target_piece) = nearest_player_piece {
            let distance = (target_piece.position - position).as_vec2().length();
            if distance < 12.0
                && let Some(best_move) =
                    self.best_hunting_move(piece_config, position, board_size, game, target_piece)
            {
                return Some(best_move);
            }
        }

        self.random_npc_move(piece_config, position, board_size, game)
    }

    /// Finds the shortest-distance hunting move toward a nearby player piece.
    fn best_hunting_move(
        &self,
        piece_config: &PieceConfig,
        position: IVec2,
        board_size: BoardSize,
        game: &GameState,
        target_piece: &Piece,
    ) -> Option<NpcMove> {
        let mut moves = self.collect_npc_moves(piece_config, position, true, board_size, game);
        moves.extend(self.collect_npc_moves(piece_config, position, false, board_size, game));
        moves.sort_by_key(|npc_move| {
            (target_piece.position - npc_move.target)
                .as_vec2()
                .length_squared() as i32
        });
        moves.into_iter().next()
    }

    /// Picks a random legal quiet move for an NPC.
    fn random_npc_move(
        &self,
        piece_config: &PieceConfig,
        position: IVec2,
        board_size: BoardSize,
        game: &GameState,
    ) -> Option<NpcMove> {
        let quiet_moves = self.collect_npc_moves(piece_config, position, false, board_size, game);
        if quiet_moves.is_empty() {
            return None;
        }

        let mut rng = rand::thread_rng();
        Some(quiet_moves[rng.gen_range(0..quiet_moves.len())])
    }

    /// Collects legal NPC moves of one kind from a piece configuration.
    fn collect_npc_moves(
        &self,
        piece_config: &PieceConfig,
        position: IVec2,
        is_capture: bool,
        board_size: BoardSize,
        game: &GameState,
    ) -> Vec<NpcMove> {
        let paths = if is_capture {
            &piece_config.capture_paths
        } else {
            &piece_config.move_paths
        };

        paths
            .iter()
            .flat_map(|path: &Vec<IVec2>| path.iter())
            .filter_map(|step| {
                let target = position + *step;
                common::logic::is_valid_move(MoveValidationParams {
                    piece_config,
                    start: BoardCoord(position),
                    end: BoardCoord(target),
                    is_capture,
                    board_size,
                    pieces: &game.pieces,
                    moving_owner: None,
                })
                .then_some(NpcMove { target, is_capture })
            })
            .collect()
    }

    /// Updates one piece's position and cooldown after a completed move.
    fn update_piece_motion(
        &self,
        piece_id: PieceId,
        target: IVec2,
        now: TimestampMs,
        piece_config: &PieceConfig,
        game: &mut GameState,
    ) {
        if let Some(piece) = game.pieces.get_mut(&piece_id) {
            piece.position = BoardCoord(target);
            piece.last_move_time = now;
            piece.cooldown_ms = piece_config.cooldown_ms;
        }
    }

    /// Returns the initial last-move time for a newly spawned NPC.
    fn initial_npc_last_move(&self, now: TimestampMs, cooldown_ms: DurationMs) -> TimestampMs {
        let cooldown_window = cooldown_ms.as_i64().max(0);
        if cooldown_window == 0 {
            return now;
        }

        let mut rng = rand::thread_rng();
        let offset = rng.gen_range(0..cooldown_window);
        TimestampMs::from_millis(now.as_i64() - offset)
    }
}
