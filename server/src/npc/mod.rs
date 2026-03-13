pub mod ai;
pub mod spawning;

use crate::state::ServerState;
use ai::find_npc_target;
use common::*;
use spawning::spawn_npcs;
use uuid::Uuid;

impl ServerState {
    pub async fn spawn_initial_shops(&self) {
        let mut game = self.game.write().await;
        spawning::spawn_initial_shops(&mut game);
    }

    pub async fn tick_npcs(&self) {
        let now = chrono::Utc::now().timestamp_millis();
        {
            let mut game = self.game.write().await;
            spawn_npcs(&mut game);
        }

        let npc_ids: Vec<Uuid> = {
            let game = self.game.read().await;
            game.pieces
                .iter()
                .filter(|(_, p)| p.owner_id.is_none())
                .map(|(id, _)| *id)
                .collect()
        };

        let mut moves_this_tick = 0;
        let max_moves_per_tick = 20;

        for id in npc_ids {
            if moves_this_tick >= max_moves_per_tick {
                break;
            }

            let (pos, cd, last_move, p_type) = {
                let game = self.game.read().await;
                match game.pieces.get(&id) {
                    Some(p) => (p.position, p.cooldown_ms, p.last_move_time, p.piece_type),
                    None => continue,
                }
            };

            if now > last_move + cd + 500 {
                let target = {
                    let game = self.game.read().await;
                    find_npc_target(&game, pos, p_type).await
                };

                if let Some(t) = target {
                    let mut game = self.game.write().await;
                    let board_size = game.board_size;

                    if !is_within_board(t, board_size) {
                        continue;
                    }

                    let target_piece = game.pieces.values().find(|pc| pc.position == t);
                    let is_friendly_npc = target_piece
                        .map(|tp| tp.owner_id.is_none())
                        .unwrap_or(false);

                    if !is_friendly_npc {
                        if p_type != PieceType::Knight && is_move_blocked(pos, t, &game.pieces) {
                            continue;
                        }

                        let capture_info = game
                            .pieces
                            .values()
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
                            let victim_stats = game
                                .players
                                .get(&cp_id)
                                .map(|p| (p.score, p.kills, p.pieces_captured, p.join_time));
                            
                            game.players.remove(&cp_id);
                            self.record_player_removal(cp_id, &mut game).await;

                            if let Some((score, kills, pieces_captured, join_time)) = victim_stats {
                                let now_ms = chrono::Utc::now().timestamp_millis();
                                let duration = ((now_ms - join_time).max(0) / 1000) as u64;
                                let channels = self.player_channels.read().await;
                                if let Some(tx) = channels.get(&cp_id) {
                                    let _ = tx.send(ServerMessage::GameOver {
                                        final_score: score,
                                        kills,
                                        pieces_captured,
                                        time_survived_secs: duration,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
