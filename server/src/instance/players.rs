//! Player join/leave logic for a game instance.

use super::GameInstance;
use crate::time::now_ms;
use common::models::{GameState, Piece, Player};
use common::protocol::{GameError, ServerMessage};
use common::types::{DurationMs, KitId, PieceId, PlayerId, Score, SessionSecret, TimestampMs};
use tokio::sync::mpsc;

impl GameInstance {
    /// Adds a player to the game, spawning their kit pieces.
    ///
    /// `name` is the display name, `kit_name` selects the kit, `tx` is the outbound channel,
    /// `pid` and `provided_secret` allow rejoining an existing player.
    /// Returns the assigned player id and session secret.
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
        if !kit.pieces.iter().any(|p| p.is_king()) {
            return Err(GameError::Internal("Kit missing king piece".to_string()));
        }

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

        let now = now_ms();
        let respawn_ms = self.mode_config.respawn_cooldown_ms;
        if respawn_ms > DurationMs::zero() {
            let deaths = self.death_timestamps.read().await;
            if let Some(death_time) = deaths.get(&player_id) {
                let elapsed = now - *death_time;
                if elapsed < respawn_ms {
                    let remaining = (respawn_ms - elapsed).as_u64() / 1000;
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
        self.victory_players.write().await.remove(&player_id);

        let mut game = self.game.write().await;
        let was_empty = game.players.is_empty();
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

        let mut king_id = None;

        {
            let mut rng = rand::thread_rng();
            for p_type_id in &kit.pieces {
                let p_id = PieceId::new();
                let p_pos = if p_type_id.is_king() {
                    king_id = Some(p_id);
                    if crate::spawning::is_free_position(&game, spawn_pos) {
                        spawn_pos
                    } else {
                        crate::spawning::find_random_nearby_free_pos(
                            &game,
                            spawn_pos,
                            &mut rng,
                            -2..=2,
                            10,
                        )
                        .unwrap_or_else(|| crate::spawning::find_spawn_pos(&game))
                    }
                } else {
                    crate::spawning::find_random_nearby_free_pos(
                        &game,
                        spawn_pos,
                        &mut rng,
                        -2..=2,
                        10,
                    )
                    .or_else(|| {
                        if crate::spawning::is_free_position(&game, spawn_pos) {
                            Some(spawn_pos)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| crate::spawning::find_spawn_pos(&game))
                };

                game.pieces.insert(
                    p_id,
                    Piece {
                        id: p_id,
                        owner_id: Some(player_id),
                        piece_type: p_type_id.clone(),
                        position: p_pos,
                        last_move_time: TimestampMs::from_millis(0),
                        cooldown_ms: DurationMs::zero(),
                    },
                );
            }
        }

        let king_id =
            king_id.ok_or_else(|| GameError::Internal("Kit missing king piece".to_string()))?;

        let player = Player {
            id: player_id,
            name,
            score: Score::zero(),
            kills: 0,
            pieces_captured: 0,
            join_time: now,
            king_id,
            color,
        };

        game.players.insert(player_id, player);
        drop(game);

        if was_empty {
            *self.last_started_at.write().await = now;
        }

        self.death_timestamps.write().await.remove(&player_id);

        Ok((player_id, session_secret))
    }

    /// Removes a player from the game and emits a final score message.
    ///
    /// `player_id` identifies the player to remove. Returns nothing.
    pub async fn remove_player(&self, player_id: PlayerId) {
        let stats = {
            let game = self.game.read().await;
            game.players
                .get(&player_id)
                .map(|p| (p.score, p.kills, p.pieces_captured, p.join_time))
        };

        if let Some((score, kills, pieces_captured, join_time)) = stats {
            let now_ms = now_ms();
            let duration = (now_ms - join_time).as_u64() / 1000;
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
        self.player_channels.write().await.remove(&player_id);
        self.victory_players.write().await.remove(&player_id);
        if self.remove_player_state(player_id, &mut game).await {
            self.record_player_leave_event().await;
        }
    }

    /// Removes a player from the game without sending a GameOver payload.
    ///
    /// Used when a player is leaving via the join flow.
    pub async fn detach_player(&self, player_id: PlayerId) {
        let mut game = self.game.write().await;
        self.player_channels.write().await.remove(&player_id);
        self.victory_players.write().await.remove(&player_id);
        if self.remove_player_state(player_id, &mut game).await {
            self.record_player_leave_event().await;
        }
    }

    /// Removes a player from active state and cleans up their owned pieces.
    ///
    /// `player_id` identifies the removed player and `game` is the mutable state to update.
    pub(super) async fn remove_player_state(
        &self,
        player_id: PlayerId,
        game: &mut GameState,
    ) -> bool {
        if game.players.remove(&player_id).is_none() {
            return false;
        }

        self.victory_players.write().await.remove(&player_id);
        self.removed_players.write().await.push(player_id);
        self.death_timestamps
            .write()
            .await
            .insert(player_id, now_ms());
        let mut rp = self.removed_pieces.write().await;
        game.pieces.retain(|id, p| {
            if p.owner_id == Some(player_id) {
                rp.push(*id);
                false
            } else {
                true
            }
        });
        true
    }
}
