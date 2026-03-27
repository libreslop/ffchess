//! Periodic tick processing for game instances.

use super::GameInstance;
use crate::time::now_ms;
use common::protocol::ServerMessage;
use common::types::{DurationMs, PieceId};
use rand::Rng;

impl GameInstance {
    /// Runs a single server tick: updates timers, broadcasts state, and cleans up.
    ///
    /// Returns nothing; this mutates game state and sends updates.
    pub async fn handle_tick(&self) {
        let now = now_ms();
        let players_viewing = !self.player_channels.read().await.is_empty()
            || !self.connection_channels.read().await.is_empty();

        if players_viewing {
            *self.last_viewed_at.write().await = now;
        }

        let last_viewed = *self.last_viewed_at.read().await;
        if now - last_viewed < DurationMs::from_millis(5000) {
            self.tick_npcs().await;
        }

        // Periodic cleanup (approx. every minute)
        static TICK_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tick = TICK_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if tick.is_multiple_of(600) {
            // Cleanup death timestamps
            let mut dt = self.death_timestamps.write().await;
            dt.retain(|_, timestamp_ms| {
                now - *timestamp_ms <= DurationMs::from_millis(10 * 60 * 1000)
            });

            let mut cm = self.color_manager.write().await;
            cm.cleanup(now.as_i64() / 1000, 24 * 60 * 60);
        }

        {
            let mut cm = self.color_manager.write().await;
            let channels = self.player_channels.read().await;
            for player_id in channels.keys() {
                cm.update_activity(*player_id);
            }
        }

        {
            let mut game = self.game.write().await;
            let target_size =
                common::logic::calculate_board_size(&self.mode_config, game.players.len());
            if target_size < game.board_size {
                let any_player_pieces_outside = game.pieces.values().any(|p| {
                    p.owner_id.is_some() && !common::logic::is_within_board(p.position, target_size)
                });

                if !any_player_pieces_outside {
                    game.board_size = target_size;
                    self.prune_out_of_bounds(&mut game).await;
                }
            }
        }

        let removed_pieces = {
            let mut rp = self.removed_pieces.write().await;
            std::mem::take(&mut *rp)
        };
        let removed_players = {
            let mut rp = self.removed_players.write().await;
            std::mem::take(&mut *rp)
        };

        let game = self.game.read().await;
        self.broadcast(ServerMessage::UpdateState {
            players: game.players.values().cloned().collect(),
            pieces: game.pieces.values().cloned().collect(),
            shops: game.shops.clone(),
            removed_pieces,
            removed_players,
            board_size: game.board_size,
        })
        .await;
    }

    /// Advances NPC spawning and movement logic for the current tick.
    ///
    /// Returns nothing; this mutates game state.
    pub async fn tick_npcs(&self) {
        let mut game = self.game.write().await;
        let board_size = game.board_size;

        // NPC Spawning
        for limit in &self.mode_config.npc_limits {
            let current_count = game
                .pieces
                .values()
                .filter(|p| {
                    p.owner_id.is_none() && p.piece_type.as_ref() == limit.piece_id.as_ref()
                })
                .count();
            let mut vars = std::collections::HashMap::new();
            vars.insert("player_count".to_string(), game.players.len() as f64);
            let max_npcs = common::logic::evaluate_expression(&limit.max_expr, &vars) as usize;

            if current_count < max_npcs {
                let spawn_pos = crate::spawning::find_spawn_pos(&game);
                let id = PieceId::new();
                game.pieces.insert(
                    id,
                    common::models::Piece {
                        id,
                        owner_id: None,
                        piece_type: limit.piece_id.clone(),
                        position: spawn_pos,
                        last_move_time: now_ms(),
                        cooldown_ms: self
                            .piece_configs
                            .get(&limit.piece_id)
                            .map(|c| c.cooldown_ms)
                            .unwrap_or_else(|| DurationMs::from_millis(2000)),
                    },
                );
            }
        }

        // NPC Movement
        let npc_ids: Vec<PieceId> = game
            .pieces
            .iter()
            .filter(|(_, p)| p.owner_id.is_none())
            .map(|(id, _)| *id)
            .collect();
        let now = now_ms();

        for id in npc_ids {
            let (p_type, p_pos, last_move, cooldown) = {
                let p = match game.pieces.get(&id) {
                    Some(p) => p,
                    None => continue,
                };
                (
                    p.piece_type.clone(),
                    p.position,
                    p.last_move_time,
                    p.cooldown_ms,
                )
            };

            if now - last_move < cooldown {
                continue;
            }

            let piece_config = match self.piece_configs.get(&p_type) {
                Some(c) => c,
                None => continue,
            };

            // Basic AI: move randomly or towards nearest player if close
            let mut moved = false;

            // Try to find a player to hunt
            let nearest_player_piece = game
                .pieces
                .values()
                .filter(|p| p.owner_id.is_some())
                .min_by_key(|p| (p.position - p_pos).as_vec2().length_squared() as i32);

            if let Some(target_p) = nearest_player_piece {
                let dist = (target_p.position - p_pos).as_vec2().length();
                if dist < 12.0 {
                    // Try to move towards target
                    // Check all possible capture/move paths
                    let mut possible_moves = Vec::new();
                    for path in &piece_config.capture_paths {
                        for step in path {
                            if common::logic::is_valid_move(common::logic::MoveValidationParams {
                                piece_config,
                                start: p_pos,
                                end: p_pos + *step,
                                is_capture: true,
                                board_size,
                                pieces: &game.pieces,
                                moving_owner: None,
                            }) {
                                possible_moves.push((p_pos + *step, true));
                            }
                        }
                    }
                    for path in &piece_config.move_paths {
                        for step in path {
                            if common::logic::is_valid_move(common::logic::MoveValidationParams {
                                piece_config,
                                start: p_pos,
                                end: p_pos + *step,
                                is_capture: false,
                                board_size,
                                pieces: &game.pieces,
                                moving_owner: None,
                            }) {
                                possible_moves.push((p_pos + *step, false));
                            }
                        }
                    }

                    if !possible_moves.is_empty() {
                        // Pick move that minimizes distance to target
                        possible_moves.sort_by_key(|(pos, _)| {
                            (target_p.position - *pos).as_vec2().length_squared() as i32
                        });
                        let (best_move, is_capture) = possible_moves[0];

                        // Execute move (re-using logic or simplifying)
                        if is_capture
                            && let Some(tp) = game
                                .pieces
                                .values()
                                .find(|p| p.position == best_move)
                                .cloned()
                        {
                            game.pieces.remove(&tp.id);
                            self.record_piece_removal(tp.id).await;
                            if tp.piece_type.is_king()
                                && let Some(owner_id) = tp.owner_id
                            {
                                // Eliminate player
                                self.record_player_removal(owner_id, &mut game).await;
                                game.players.remove(&owner_id);
                            }
                        }

                        if let Some(p) = game.pieces.get_mut(&id) {
                            p.position = best_move;
                            p.last_move_time = now;
                            p.cooldown_ms = piece_config.cooldown_ms;
                            moved = true;
                        }
                    }
                }
            }

            if !moved {
                // Random move
                let mut all_steps = Vec::new();
                for path in &piece_config.move_paths {
                    for step in path {
                        all_steps.push(*step);
                    }
                }
                if !all_steps.is_empty() {
                    let step = {
                        let mut rng = rand::thread_rng();
                        all_steps[rng.gen_range(0..all_steps.len())]
                    };
                    let target = p_pos + step;
                    if common::logic::is_valid_move(common::logic::MoveValidationParams {
                        piece_config,
                        start: p_pos,
                        end: target,
                        is_capture: false,
                        board_size,
                        pieces: &game.pieces,
                        moving_owner: None,
                    }) && let Some(p) = game.pieces.get_mut(&id)
                    {
                        p.position = target;
                        p.last_move_time = now;
                        p.cooldown_ms = piece_config.cooldown_ms;
                    }
                }
            }
        }
    }
}
