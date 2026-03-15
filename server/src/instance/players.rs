use super::GameInstance;
use common::models::{GameState, Piece, Player};
use common::protocol::{GameError, ServerMessage};
use common::types::{KitId, PieceId, PieceTypeId, PlayerId, SessionSecret};
use glam::IVec2;
use rand::Rng;
use tokio::sync::mpsc;

impl GameInstance {
    pub async fn add_player(
        &self,
        name: String,
        kit_name: KitId,
        tx: mpsc::UnboundedSender<ServerMessage>,
        pid: Option<PlayerId>,
        provided_secret: Option<SessionSecret>,
    ) -> Result<(PlayerId, SessionSecret), GameError> {
        let kit = self
            .mode_config
            .kits
            .iter()
            .find(|k| k.name == kit_name)
            .ok_or_else(|| GameError::Internal("Kit not found".to_string()))?;

        let player_id = pid.unwrap_or_default();

        let mut secrets = self.session_secrets.write().await;
        let session_secret = if let Some(stored_secret) = secrets.get(&player_id) {
            if Some(*stored_secret) != provided_secret {
                return Err(GameError::Custom {
                    title: "SESSION ERROR".to_string(),
                    message: "Invalid session secret for this player ID.".to_string(),
                });
            }
            *stored_secret
        } else {
            let new_secret = SessionSecret::new();
            secrets.insert(player_id, new_secret);
            new_secret
        };
        drop(secrets);

        let respawn_ms = self.mode_config.respawn_cooldown_ms as i64;
        if respawn_ms > 0 {
            let deaths = self.death_timestamps.read().await;
            if let Some(death_time) = deaths.get(&player_id) {
                let now = chrono::Utc::now().timestamp_millis();
                let elapsed = now - death_time;
                if elapsed < respawn_ms {
                    let remaining = (respawn_ms - elapsed) / 1000;
                    return Err(GameError::Custom {
                        title: "Respawn cooldown".to_string(),
                        message: format!("Wait {} seconds", remaining.max(1)),
                    });
                }
            }
        }

        let color = {
            let active_ids: Vec<PlayerId> =
                self.player_channels.read().await.keys().cloned().collect();
            let mut cm = self.color_manager.write().await;
            cm.get_or_assign_color(player_id, &active_ids)
        };

        self.player_channels.write().await.insert(player_id, tx);

        let mut game = self.game.write().await;
        let new_size =
            common::logic::calculate_board_size(&self.mode_config, game.players.len() + 1);
        if new_size > game.board_size {
            game.board_size = new_size;
            self.prune_out_of_bounds(&mut game).await;
        }

        let spawn_pos = crate::spawning::find_spawn_pos(&game);

        // Clear NPCs near spawn
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

        let mut king_id = PieceId::nil(); // Placeholder
        let king_type = PieceTypeId::from("king");

        {
            let mut rng = rand::thread_rng();
            for p_type_id in &kit.pieces {
                let p_id = PieceId::new();
                let mut p_pos = spawn_pos;

                if p_type_id == &king_type {
                    king_id = p_id;
                    // Try to put king at center
                    if !game.pieces.values().any(|p| p.position == spawn_pos) {
                        p_pos = spawn_pos;
                    } else {
                        // find nearby
                        for _ in 0..10 {
                            let offset = IVec2::new(rng.gen_range(-2..=2), rng.gen_range(-2..=2));
                            let candidate = spawn_pos + offset;
                            if candidate != spawn_pos
                                && common::logic::is_within_board(candidate, game.board_size)
                                && !game.pieces.values().any(|p| p.position == candidate)
                                && !game.shops.iter().any(|s| s.position == candidate)
                            {
                                p_pos = candidate;
                                break;
                            }
                        }
                    }
                } else {
                    for _ in 0..10 {
                        let offset = IVec2::new(rng.gen_range(-2..=2), rng.gen_range(-2..=2));
                        let candidate = spawn_pos + offset;
                        if candidate != spawn_pos // Reserve exact center for king if possible (though loop order matters)
                            && common::logic::is_within_board(candidate, game.board_size)
                            && !game.pieces.values().any(|p| p.position == candidate)
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
                        piece_type: p_type_id.clone(),
                        position: p_pos,
                        last_move_time: 0,
                        cooldown_ms: 0,
                    },
                );
            }
        }

        // Fallback if kit didn't have a king (shouldn't happen with valid config)
        if king_id == PieceId::nil() {
            king_id = PieceId::new();
            game.pieces.insert(
                king_id,
                Piece {
                    id: king_id,
                    owner_id: Some(player_id),
                    piece_type: king_type,
                    position: spawn_pos,
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
            join_time: chrono::Utc::now().timestamp_millis(),
            king_id,
            color,
        };

        game.players.insert(player_id, player);
        drop(game);

        self.death_timestamps.write().await.remove(&player_id);

        Ok((player_id, session_secret))
    }

    pub async fn remove_player(&self, player_id: PlayerId) {
        let stats = {
            let game = self.game.read().await;
            game.players
                .get(&player_id)
                .map(|p| (p.score, p.kills, p.pieces_captured, p.join_time))
        };

        if let Some((score, kills, pieces_captured, join_time)) = stats {
            let now_ms = chrono::Utc::now().timestamp_millis();
            let duration = ((now_ms - join_time).max(0) / 1000) as u64;
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
        if game.players.remove(&player_id).is_some() {
            self.player_channels.write().await.remove(&player_id);
            self.record_player_removal(player_id, &mut game).await;
        }
    }

    pub async fn record_player_removal(&self, player_id: PlayerId, game: &mut GameState) {
        self.removed_players.write().await.push(player_id);
        self.death_timestamps
            .write()
            .await
            .insert(player_id, chrono::Utc::now().timestamp_millis());
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
