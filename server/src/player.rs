use crate::spawning::find_spawn_pos;
use crate::state::ServerState;
use common::*;
use glam::IVec2;
use rand::Rng;
use uuid::Uuid;

impl ServerState {
    pub async fn add_player(
        &self,
        name: String,
        kit: KitType,
        tx: tokio::sync::mpsc::UnboundedSender<ServerMessage>,
        existing_id: Option<Uuid>,
    ) -> Result<Uuid, GameError> {
        let player_id = existing_id.unwrap_or_else(Uuid::new_v4);

        {
            let deaths = self.death_timestamps.read().await;
            if let Some(death_time) = deaths.get(&player_id) {
                let now = chrono::Utc::now().timestamp_millis();
                let game = self.game.read().await;
                let elapsed = now - death_time;
                if elapsed < game.respawn_cooldown_ms as i64 {
                    let remaining = (game.respawn_cooldown_ms as i64 - elapsed) / 1000;
                    return Err(GameError::Custom {
                        title: "RESPAWN COOLDOWN".to_string(),
                        message: format!("You must wait {} more seconds to respawn.", remaining.max(1)),
                    });
                }
            }
        }

        let color = {
            let active_ids: Vec<Uuid> = self.player_channels.read().await.keys().cloned().collect();
            let mut cm = self.color_manager.write().await;
            cm.get_or_assign_color(player_id, &active_ids)
        };

        self.player_channels.write().await.insert(player_id, tx);

        let mut game = self.game.write().await;

        let new_size = calculate_board_size(game.players.len() + 1);
        if new_size > game.board_size {
            game.board_size = new_size;
            self.prune_out_of_bounds(&mut game).await;
        }

        let spawn_pos = find_spawn_pos(&game);

        {
            let mut rp = self.removed_pieces.write().await;
            game.pieces.retain(|id, p| {
                if p.owner_id.is_none() && (p.position - spawn_pos).as_vec2().length() <= 15.0 {
                    rp.push(*id);
                    false
                } else {
                    true
                }
            });
        }

        let king_id = Uuid::new_v4();
        game.pieces.insert(
            king_id,
            Piece {
                id: king_id,
                owner_id: Some(player_id),
                piece_type: PieceType::King,
                position: spawn_pos,
                last_move_time: 0,
                cooldown_ms: 0,
            },
        );

        let mut rng = rand::thread_rng();
        for p_type in kit.get_pieces() {
            let p_id = Uuid::new_v4();
            let mut p_pos = spawn_pos;
            for _ in 0..10 {
                let offset = IVec2::new(rng.gen_range(-2..=2), rng.gen_range(-2..=2));
                let candidate = spawn_pos + offset;
                if candidate != spawn_pos
                    && is_within_board(candidate, game.board_size)
                    && !game.pieces.values().any(|p| p.position == candidate)
                    && !game.shops.iter().any(|s| s.position == candidate)
                {
                    p_pos = candidate;
                    break;
                }
            }

            if p_pos == spawn_pos {
                let neighbors = [
                    IVec2::new(1, 0),
                    IVec2::new(-1, 0),
                    IVec2::new(0, 1),
                    IVec2::new(0, -1),
                    IVec2::new(1, 1),
                    IVec2::new(-1, 1),
                    IVec2::new(1, -1),
                    IVec2::new(-1, -1),
                ];
                for offset in neighbors {
                    let candidate = spawn_pos + offset;
                    if is_within_board(candidate, game.board_size)
                        && !game.shops.iter().any(|s| s.position == candidate)
                    {
                        p_pos = candidate;
                        break;
                    }
                }
            }

            game.pieces.insert(
                p_id,
                Piece {
                    id: p_id,
                    owner_id: Some(player_id),
                    piece_type: p_type,
                    position: p_pos,
                    last_move_time: 0,
                    cooldown_ms: 0,
                },
            );
        }

        let player = Player {
            id: player_id,
            name,
            score: 0,
            kills: 0,
            pieces_captured: 0,
            join_time: chrono::Utc::now().timestamp(),
            king_id,
            color,
        };

        game.players.insert(player_id, player);
        Ok(player_id)
    }

    pub async fn remove_player(&self, player_id: Uuid) {
        let stats = {
            let game = self.game.read().await;
            game.players
                .get(&player_id)
                .map(|p| (p.score, p.kills, p.pieces_captured, p.join_time))
        };

        if let Some((score, kills, pieces_captured, join_time)) = stats {
            let now = chrono::Utc::now().timestamp();
            let duration = (now - join_time).max(0) as u64;
            let channels = self.player_channels.read().await;
            if let Some(tx) = channels.get(&player_id) {
                let _ = tx.send(ServerMessage::GameOver {
                    final_score: score,
                    kills,
                    pieces_captured,
                    time_survived_secs: duration,
                });
            }
        }

        let mut game = self.game.write().await;
        game.players.remove(&player_id);
        self.player_channels.write().await.remove(&player_id);
        self.record_player_removal(player_id, &mut game).await;
    }

    pub async fn record_player_removal(&self, player_id: Uuid, game: &mut GameState) {
        self.removed_players.write().await.push(player_id);
        self.death_timestamps.write().await.insert(player_id, chrono::Utc::now().timestamp_millis());
        let mut rp = self.removed_pieces.write().await;
        game.pieces.retain(|id, p| {
            if p.owner_id == Some(player_id) {
                rp.push(*id);
                false
            } else {
                true
            }
        });
    }
}
