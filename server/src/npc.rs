use common::*;
use crate::state::ServerState;
use rand::Rng;
use glam::IVec2;
use uuid::Uuid;

impl ServerState {
    pub async fn spawn_initial_shops(&self) {
        let mut game = self.game.write().await;
        for _ in 0..10 {
            Self::spawn_random_shop(&mut game);
        }
    }

    pub fn spawn_random_shop(game: &mut GameState) {
        let board_size = game.board_size;
        let mut rng = rand::thread_rng();
        let half = board_size / 2;
        let limit = (board_size + 1) / 2;
        game.shops.push(Shop {
            position: IVec2::new(rng.gen_range(-half..limit), rng.gen_range(-half..limit)),
            uses_remaining: 1, // Shops are now single-use
            shop_type: if rng.gen_bool(0.5) { ShopType::Spawn } else { ShopType::Upgrade },
        });
    }

    pub async fn tick_npcs(&self) {
        let mut game = self.game.write().await;
        let board_size = game.board_size;
        let half = board_size / 2;
        let limit = (board_size + 1) / 2;
        
        // Reduce target density (e.g., 1 per 250 squares instead of 100)
        let target_npc_count = (board_size * board_size / 250).clamp(20, 200);

        {
            let mut rng = rand::thread_rng();
            if game.pieces.values().filter(|p| p.owner_id.is_none()).count() < target_npc_count as usize {
                let id = Uuid::new_v4();
                let pos = IVec2::new(rng.gen_range(-half..limit), rng.gen_range(-half..limit));
                
                // Don't spawn NPC too close to any player
                let too_close = game.pieces.values().any(|p| {
                    p.owner_id.is_some() && (p.position - pos).abs().max_element() <= 10
                });

                if !too_close {
                    let p_type = match rng.gen_range(0..100) {
                        0..=75 => PieceType::Pawn,
                        76..=88 => PieceType::Knight,
                        89..=96 => PieceType::Bishop,
                        97..=99 => PieceType::Rook,
                        _ => PieceType::Queen,
                    };
                    
                    game.pieces.insert(id, Piece {
                        id,
                        owner_id: None,
                        piece_type: p_type,
                        position: pos,
                        last_move_time: 0,
                        cooldown_ms: 0,
                    });
                }
            }
        }
        
        let npc_ids: Vec<Uuid> = game.pieces.iter()
            .filter(|(_, p)| p.owner_id.is_none())
            .map(|(id, _)| *id)
            .collect();

        let now = chrono::Utc::now().timestamp_millis();
        let mut moves_this_tick = 0;
        let max_moves_per_tick = 20;

        for id in npc_ids {
            if moves_this_tick >= max_moves_per_tick {
                break;
            }

            let (pos, cd, last_move, p_type) = {
                let p = game.pieces.get(&id).unwrap();
                (p.position, p.cooldown_ms, p.last_move_time, p.piece_type)
            };

            if now > last_move + cd + 500 {
                // 1. Check if visible to any player
                let mut visible_to_player = None;
                for player in game.players.values() {
                    if let Some(king) = game.pieces.get(&player.king_id) {
                        let p_piece_count = game.pieces.values().filter(|p| p.owner_id == Some(player.id)).count();
                        let view_radius = (10.0 * (p_piece_count as f64).sqrt().max(1.0)) as i32;
                        if (pos - king.position).abs().max_element() <= view_radius {
                            visible_to_player = Some(player.id);
                            break;
                        }
                    }
                }

                let mut target = None;

                // Only engage if a player is relatively close (12 squares)
                let mut player_nearby = false;
                if let Some(pid) = visible_to_player {
                    for piece in game.pieces.values() {
                        if piece.owner_id == Some(pid) {
                            if (piece.position - pos).abs().max_element() <= 12 {
                                player_nearby = true;
                                break;
                            }
                        }
                    }
                }

                if player_nearby {
                    // 2. Aggressive Mode: PRIORITIZE King captures, then other player pieces
                    let mut king_target = None;
                    let mut other_target = None;
                    
                    for other_p in game.pieces.values() {
                        if other_p.owner_id.is_some() {
                            let dist = (other_p.position - pos).abs();
                            // Reduced capture range from 15 to 10
                            if dist.max_element() <= 10
                                && is_within_board(other_p.position, board_size) // Boundary check
                                && is_valid_chess_move(p_type, pos, other_p.position, true, board_size)
                                && (p_type == PieceType::Knight || !is_move_blocked(pos, other_p.position, &game.pieces)) {
                                if other_p.piece_type == PieceType::King {
                                    king_target = Some(other_p.position);
                                    break; // King found! Immediate priority.
                                } else {
                                    other_target = Some(other_p.position);
                                }
                            }
                        }
                    }

                    if let Some(t) = king_target {
                        target = Some(t);
                    } else if let Some(t) = other_target {
                        target = Some(t);
                    } else {
                        // 3. Hunt Mode: Move toward the nearest player piece (PRIORITIZE King)
                        let mut nearest_p_pos = None;
                        let mut min_dist = f32::MAX;
                        
                        // First, check for Kings
                        for player in game.players.values() {
                            if let Some(king) = game.pieces.get(&player.king_id) {
                                if !is_within_board(king.position, board_size) { continue; } // Boundary check
                                let d = (king.position - pos).as_vec2().length();
                                // Reduced hunt range from 18 to 12
                                if d < 12.0 {
                                    min_dist = d;
                                    nearest_p_pos = Some(king.position);
                                    break;
                                }
                            }
                        }
                        
                        // If no King nearby, check other pieces
                        if nearest_p_pos.is_none() {
                            for other_p in game.pieces.values() {
                                if other_p.owner_id.is_some() {
                                    if !is_within_board(other_p.position, board_size) { continue; } // Boundary check
                                    let d = (other_p.position - pos).as_vec2().length();
                                    // Reduced hunt range from 18 to 12
                                    if d < 12.0 && d < min_dist {
                                        min_dist = d;
                                        nearest_p_pos = Some(other_p.position);
                                    }
                                }
                            }
                        }

                        if let Some(npp) = nearest_p_pos {
                            let mut best_move = None;
                            let mut best_dist = min_dist;
                            let range = 4;
                            for dx in -range..=range {
                                for dy in -range..=range {
                                    let t = pos + IVec2::new(dx, dy);
                                    if is_within_board(t, board_size)
                                        && is_valid_chess_move(p_type, pos, t, false, board_size)
                                        && (p_type == PieceType::Knight || !is_move_blocked(pos, t, &game.pieces))
                                        && !game.pieces.values().any(|pc| pc.position == t) {
                                        let d = (npp - t).as_vec2().length();
                                        if d < best_dist {
                                            best_dist = d;
                                            best_move = Some(t);
                                        }
                                    }
                                }
                            }
                            target = best_move;
                        }
                    }
                }

                if target.is_none() {
                    // 4. Roam Mode (Fallback)
                    target = match p_type {
                        PieceType::Pawn => {
                            let mut rng = rand::thread_rng();
                            let directions = [
                                IVec2::new(0, -1),
                                IVec2::new(1, 0),
                                IVec2::new(0, 1),
                                IVec2::new(-1, 0),
                            ];
                            let dir = directions[rng.gen_range(0..4)];
                            let t = pos + dir;
                            if is_within_board(t, board_size) && !game.pieces.values().any(|pc| pc.position == t) {
                                Some(t)
                            } else {
                                None
                            }
                        },
                        _ => {
                            let mut potential_targets = Vec::new();
                            let range = 2;
                            for dx in -range..=range {
                                for dy in -range..=range {
                                    let t = pos + IVec2::new(dx, dy);
                                    if is_within_board(t, board_size)
                                        && is_valid_chess_move(p_type, pos, t, false, board_size)
                                        && (p_type == PieceType::Knight || !is_move_blocked(pos, t, &game.pieces))
                                        && !game.pieces.values().any(|pc| pc.position == t) {
                                        potential_targets.push(t);
                                    }
                                }
                            }
                            if !potential_targets.is_empty() {
                                let mut rng = rand::thread_rng();
                                Some(potential_targets[rng.gen_range(0..potential_targets.len())])
                            } else {
                                None
                            }
                        }
                    };
                }

                if let Some(t) = target
                    && is_within_board(t, board_size) {
                    let target_piece = game.pieces.values().find(|pc| pc.position == t);
                    let is_friendly_npc = target_piece.map(|tp| tp.owner_id.is_none()).unwrap_or(false);
                    
                    if !is_friendly_npc {
                        let mut blocked = false;
                        if p_type != PieceType::Knight {
                            blocked = is_move_blocked(pos, t, &game.pieces);
                        }

                        if !blocked {
                            let capture_info = game.pieces.values()
                                .find(|pc| pc.position == t)
                                .map(|tp| (tp.id, tp.piece_type, tp.owner_id));

                            let mut captured_player_id = None;
                            if let Some((tid, t_type, towner)) = capture_info {
                                if t_type == PieceType::King {
                                    captured_player_id = towner;
                                }
                                game.pieces.remove(&tid);
                                self.record_piece_removal(tid).await;
                            }

                            let config = game.cooldown_config.clone();
                            if let Some(p) = game.pieces.get_mut(&id) {
                                p.position = t;
                                p.last_move_time = now;
                                p.cooldown_ms = calculate_cooldown(p_type, pos, t, &config);
                                moves_this_tick += 1;
                            }

                            if let Some(cp_id) = captured_player_id {
                                game.players.remove(&cp_id);
                                self.record_player_removal(cp_id, &mut game).await;
                                // NPCs don't have a kill count, but we could add it to piece if we wanted.
                                // For now just removing the player is enough.
                            }
                        }
                    }
                }
            }
        }
    }
}
