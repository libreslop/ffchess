use crate::colors::ColorManager;
use common::*;
use common::models::{GameModeConfig, GameState, Piece, PieceConfig, Player, Shop, ShopConfig};
use common::protocol::{GameError, ServerMessage};
use glam::IVec2;
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

pub struct GameInstance {
    pub mode_config: GameModeConfig,
    pub piece_configs: Arc<HashMap<String, PieceConfig>>,
    pub shop_configs: Arc<HashMap<String, ShopConfig>>,
    pub game: RwLock<GameState>,
    pub player_channels: RwLock<HashMap<Uuid, mpsc::UnboundedSender<ServerMessage>>>,
    pub session_secrets: RwLock<HashMap<Uuid, Uuid>>,
    pub removed_pieces: RwLock<Vec<Uuid>>,
    pub removed_players: RwLock<Vec<Uuid>>,
    pub color_manager: RwLock<ColorManager>,
    pub last_viewed_at: RwLock<i64>,
    pub death_timestamps: RwLock<HashMap<Uuid, i64>>,
}

impl GameInstance {
    pub fn new(
        mode_config: GameModeConfig,
        piece_configs: Arc<HashMap<String, PieceConfig>>,
        shop_configs: Arc<HashMap<String, ShopConfig>>,
    ) -> Self {
        let board_size = common::logic::calculate_board_size(&mode_config, 0);
        Self {
            mode_config: mode_config.clone(),
            piece_configs,
            shop_configs,
            game: RwLock::new(GameState {
                board_size,
                mode_id: mode_config.id.clone(),
                ..Default::default()
            }),
            player_channels: RwLock::new(HashMap::new()),
            session_secrets: RwLock::new(HashMap::new()),
            removed_pieces: RwLock::new(Vec::new()),
            removed_players: RwLock::new(Vec::new()),
            color_manager: RwLock::new(ColorManager::new()),
            last_viewed_at: RwLock::new(chrono::Utc::now().timestamp_millis()),
            death_timestamps: RwLock::new(HashMap::new()),
        }
    }

    pub async fn broadcast(&self, msg: ServerMessage) {
        let channels = self.player_channels.read().await;
        for tx in channels.values() {
            let _ = tx.send(msg.clone());
        }
    }

    pub async fn record_piece_removal(&self, piece_id: Uuid) {
        self.removed_pieces.write().await.push(piece_id);
    }

    pub async fn add_player(
        &self,
        name: String,
        kit_name: String,
        tx: mpsc::UnboundedSender<ServerMessage>,
        pid: Option<Uuid>,
        provided_secret: Option<Uuid>,
    ) -> Result<(Uuid, Uuid), GameError> {
        let kit = self
            .mode_config
            .kits
            .iter()
            .find(|k| k.name == kit_name)
            .ok_or_else(|| GameError::Internal("Kit not found".to_string()))?;

        let player_id = pid.unwrap_or_else(Uuid::new_v4);

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
            let new_secret = Uuid::new_v4();
            secrets.insert(player_id, new_secret);
            new_secret
        };
        drop(secrets);

        {
            let deaths = self.death_timestamps.read().await;
            if let Some(death_time) = deaths.get(&player_id) {
                let now = chrono::Utc::now().timestamp_millis();
                let elapsed = now - death_time;
                if elapsed < 5000 {
                    // Hardcoded 5s respawn cooldown for now
                    let remaining = (5000 - elapsed) / 1000;
                    return Err(GameError::Custom {
                        title: "Respawn cooldown".to_string(),
                        message: format!("Wait {} seconds", remaining.max(1)),
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
        let new_size = common::logic::calculate_board_size(&self.mode_config, game.players.len() + 1);
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

        let mut king_id = Uuid::nil(); // Placeholder

        {
            let mut rng = rand::thread_rng();
            for p_type_id in &kit.pieces {
                let p_id = Uuid::new_v4();
                let mut p_pos = spawn_pos;
                
                // If it's a king, we try to place it exactly at spawn_pos if possible, 
                // but the loop below handles collision. 
                // Actually, let's prioritize king at spawn_pos.
                
                if p_type_id == "king" {
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
        if king_id == Uuid::nil() {
            king_id = Uuid::new_v4();
            game.pieces.insert(
                king_id,
                Piece {
                    id: king_id,
                    owner_id: Some(player_id),
                    piece_type: "king".to_string(),
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

    pub async fn remove_player(&self, player_id: Uuid) {
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

    pub async fn record_player_removal(&self, player_id: Uuid, game: &mut GameState) {
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

    pub async fn prune_out_of_bounds(&self, game: &mut GameState) {
        let board_size = game.board_size;
        let mut rp = self.removed_pieces.write().await;
        game.pieces.retain(|id, p| {
            if !common::logic::is_within_board(p.position, board_size) {
                rp.push(*id);
                false
            } else {
                true
            }
        });
        game.shops
            .retain(|s| common::logic::is_within_board(s.position, board_size));
    }

    pub async fn handle_move(
        &self,
        player_id: Uuid,
        piece_id: Uuid,
        target: IVec2,
    ) -> Result<(), GameError> {
        let mut game = self.game.write().await;
        let board_size = game.board_size;

        let (piece_type, start_pos, piece_owner, _cooldown) = {
            let piece = game.pieces.get(&piece_id).ok_or(GameError::PieceNotFound)?;
            if piece.owner_id != Some(player_id) {
                return Err(GameError::NotYourPiece);
            }
            let now = chrono::Utc::now().timestamp_millis();
            let elapsed = now - piece.last_move_time;
            if elapsed < piece.cooldown_ms {
                return Err(GameError::OnCooldown);
            }
            (piece.piece_type.clone(), piece.position, piece.owner_id, piece.cooldown_ms)
        };

        let target_piece = game.pieces.values().find(|p| p.position == target).cloned();
        let is_capture = if let Some(ref tp) = target_piece {
            if tp.owner_id == Some(player_id) {
                return Err(GameError::TargetFriendly);
            }
            true
        } else {
            false
        };

        let piece_config = self
            .piece_configs
            .get(&piece_type)
            .ok_or_else(|| GameError::Internal("Piece config not found".to_string()))?;

        if !common::logic::is_valid_move(
            piece_config,
            start_pos,
            target,
            is_capture,
            game.board_size,
            &game.pieces,
            piece_owner,
        ) {

            return Err(GameError::InvalidMove);
        }

        // Apply move
        if let Some(tp) = target_piece {
            game.pieces.remove(&tp.id);
            self.record_piece_removal(tp.id).await;

            let attacker_score = self
                .piece_configs
                .get(&tp.piece_type)
                .map(|c| c.score_value)
                .unwrap_or(0);

            if let Some(player) = game.players.get_mut(&player_id) {
                player.score += attacker_score;
                player.pieces_captured += 1;
                if tp.piece_type == "king" {
                    player.kills += 1;
                }
            }

            // Handle hooks (e.g., EliminateOwner)
            for hook in &self.mode_config.hooks {
                if hook.trigger == "OnCapture" && hook.target_piece_id == tp.piece_type {
                    if let Some(target_owner_id) = tp.owner_id {
                        if hook.action == "EliminateOwner" {
                            // Record player removal (updates death timestamps)
                            self.record_player_removal(target_owner_id, &mut *game).await;
                            game.players.remove(&target_owner_id);
                        }
                    }
                }
            }
        }

        if let Some(piece) = game.pieces.get_mut(&piece_id) {
            piece.position = target;
            piece.last_move_time = chrono::Utc::now().timestamp_millis();
            
            // Calculate cooldown
            let dist = (target - start_pos).as_vec2().length() as f64;
            // For now, let's just use a simple cooldown logic: base + dist * factor
            // Actually, we should probably have a more complex formula in the JSON.
            // But the JSON only has cooldown_ms. Let's use it as a base.
            piece.cooldown_ms = piece_config.cooldown_ms as i64;
        }

        Ok(())
    }

    pub async fn handle_shop_buy(
        &self,
        player_id: Uuid,
        shop_pos: IVec2,
        item_index: usize,
    ) -> Result<(), GameError> {
        let mut game = self.game.write().await;
        let (shop_id, shop_index) = game
            .shops
            .iter()
            .enumerate()
            .find(|(_, s)| s.position == shop_pos)
            .map(|(i, s)| (s.shop_id.clone(), i))
            .ok_or(GameError::ShopNotFound)?;

        let shop_config = self
            .shop_configs
            .get(&shop_id)
            .ok_or_else(|| GameError::Internal("Shop config not found".to_string()))?;

        let player_piece_on_shop = game
            .pieces
            .values()
            .find(|p| p.position == shop_pos && p.owner_id == Some(player_id))
            .cloned();

        let group = if let Some(ref p) = player_piece_on_shop {
            shop_config
                .groups
                .iter()
                .find(|g| g.applies_to.contains(&p.piece_type))
                .unwrap_or(&shop_config.default_group)
        } else {
            &shop_config.default_group
        };

        let item = group.items.get(item_index).ok_or(GameError::Internal("Invalid shop item index".to_string()))?;

        // Evaluate price
        let mut vars = HashMap::new();
        vars.insert("player_piece_count".to_string(), game.pieces.values().filter(|p| p.owner_id == Some(player_id)).count() as f64);
        // Add specific piece counts
        for p_id in self.piece_configs.keys() {
            let count = game.pieces.values().filter(|p| p.owner_id == Some(player_id) && &p.piece_type == p_id).count();
            vars.insert(format!("{}_count", p_id), count as f64);
        }

        let price = common::logic::evaluate_expression(&item.price_expr, &vars) as u64;

        let player = game.players.get_mut(&player_id).ok_or(GameError::PlayerNotFound)?;
        if player.score < price {
            return Err(GameError::InsufficientScore {
                needed: price,
                have: player.score,
            });
        }

        // Deduct score
        player.score -= price;

        // Apply item
        if let Some(ref replace_type) = item.replace_with {
            if let Some(mut p) = player_piece_on_shop {
                if let Some(piece) = game.pieces.get_mut(&p.id) {
                    piece.piece_type = replace_type.clone();
                    piece.cooldown_ms = self.piece_configs.get(replace_type).map(|c| c.cooldown_ms as i64).unwrap_or(1000);
                }
            }
        }

        for add_type in &item.add_pieces {
            let p_id = Uuid::new_v4();
            let mut p_pos = shop_pos;
            // Find nearby space
            let neighbors = [
                IVec2::new(1, 0), IVec2::new(-1, 0), IVec2::new(0, 1), IVec2::new(0, -1),
                IVec2::new(1, 1), IVec2::new(-1, 1), IVec2::new(1, -1), IVec2::new(-1, -1),
            ];
            let mut found = false;
            for offset in neighbors {
                let candidate = shop_pos + offset;
                if common::logic::is_within_board(candidate, game.board_size)
                    && !game.pieces.values().any(|p| p.position == candidate)
                    && !game.shops.iter().any(|s| s.position == candidate)
                {
                    p_pos = candidate;
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(GameError::NoSpaceNearby);
            }

            game.pieces.insert(p_id, Piece {
                id: p_id,
                owner_id: Some(player_id),
                piece_type: add_type.clone(),
                position: p_pos,
                last_move_time: 0,
                cooldown_ms: 0,
            });
        }

        // Deplete shop
        game.shops[shop_index].uses_remaining -= 1;
        if game.shops[shop_index].uses_remaining == 0 {
            game.shops.remove(shop_index);
            // Spawn a new one elsewhere
            let new_pos = crate::spawning::find_spawn_pos(&game);
            game.shops.push(Shop {
                position: new_pos,
                uses_remaining: shop_config.default_uses,
                shop_id: shop_id.clone(),
            });
        }

        Ok(())
    }

    pub async fn handle_tick(&self) {
        let now = chrono::Utc::now().timestamp_millis();
        let players_viewing = !self.player_channels.read().await.is_empty();

        if players_viewing {
            *self.last_viewed_at.write().await = now;
        }

        let last_viewed = *self.last_viewed_at.read().await;
        if now - last_viewed < 5000 {
            self.tick_npcs().await;
        }

        // Periodic cleanup (approx. every minute)
        static TICK_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tick = TICK_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if tick % 600 == 0 {
            // Cleanup death timestamps
            let mut dt = self.death_timestamps.write().await;
            dt.retain(|_, timestamp_ms| now - *timestamp_ms <= 10 * 60 * 1000);
            
            let mut cm = self.color_manager.write().await;
            cm.cleanup(now / 1000, 24 * 60 * 60);
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
            let target_size = common::logic::calculate_board_size(&self.mode_config, game.players.len());
            if target_size < game.board_size {
                let any_player_pieces_outside = game
                    .pieces
                    .values()
                    .any(|p| p.owner_id.is_some() && !common::logic::is_within_board(p.position, target_size));

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

    pub async fn tick_npcs(&self) {
        let mut game = self.game.write().await;
        let board_size = game.board_size;

        // NPC Spawning
        for limit in &self.mode_config.npc_limits {
            let current_count = game.pieces.values().filter(|p| p.owner_id.is_none() && p.piece_type == limit.piece_id).count();
            let mut vars = HashMap::new();
            vars.insert("player_count".to_string(), game.players.len() as f64);
            let max_npcs = common::logic::evaluate_expression(&limit.max_expr, &vars) as usize;

            if current_count < max_npcs {
                let spawn_pos = crate::spawning::find_spawn_pos(&game);
                let id = Uuid::new_v4();
                game.pieces.insert(id, Piece {
                    id,
                    owner_id: None,
                    piece_type: limit.piece_id.clone(),
                    position: spawn_pos,
                    last_move_time: chrono::Utc::now().timestamp_millis(),
                    cooldown_ms: self.piece_configs.get(&limit.piece_id).map(|c| c.cooldown_ms as i64).unwrap_or(2000),
                });
            }
        }

        // NPC Movement
        let npc_ids: Vec<Uuid> = game.pieces.iter().filter(|(_, p)| p.owner_id.is_none()).map(|(id, _)| *id).collect();
        let now = chrono::Utc::now().timestamp_millis();

        for id in npc_ids {
            let (p_type, p_pos, last_move, cooldown) = {
                let p = match game.pieces.get(&id) {
                    Some(p) => p,
                    None => continue,
                };
                (p.piece_type.clone(), p.position, p.last_move_time, p.cooldown_ms)
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
            let nearest_player_piece = game.pieces.values()
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
                            if common::logic::is_valid_move(piece_config, p_pos, p_pos + *step, true, board_size, &game.pieces, None) {
                                possible_moves.push((p_pos + *step, true));
                            }
                        }
                    }
                    for path in &piece_config.move_paths {
                        for step in path {
                            if common::logic::is_valid_move(piece_config, p_pos, p_pos + *step, false, board_size, &game.pieces, None) {
                                possible_moves.push((p_pos + *step, false));
                            }
                        }
                    }

                    if !possible_moves.is_empty() {
                        // Pick move that minimizes distance to target
                        possible_moves.sort_by_key(|(pos, _)| (target_p.position - *pos).as_vec2().length_squared() as i32);
                        let (best_move, is_capture) = possible_moves[0];
                        
                        // Execute move (re-using logic or simplifying)
                        if is_capture {
                            if let Some(tp) = game.pieces.values().find(|p| p.position == best_move).cloned() {
                                game.pieces.remove(&tp.id);
                                self.record_piece_removal(tp.id).await;
                                if tp.piece_type == "king" {
                                    if let Some(owner_id) = tp.owner_id {
                                        // Eliminate player
                                        self.record_player_removal(owner_id, &mut *game).await;
                                        game.players.remove(&owner_id);
                                    }
                                }
                            }
                        }

                        if let Some(p) = game.pieces.get_mut(&id) {
                            p.position = best_move;
                            p.last_move_time = now;
                            p.cooldown_ms = piece_config.cooldown_ms as i64;
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
                    if common::logic::is_valid_move(piece_config, p_pos, target, false, board_size, &game.pieces, None) {
                        if let Some(p) = game.pieces.get_mut(&id) {
                            p.position = target;
                            p.last_move_time = now;
                            p.cooldown_ms = piece_config.cooldown_ms as i64;
                        }
                    }
                }
            }
        }
    }

    pub async fn spawn_initial_shops(&self) {
        let mut game = self.game.write().await;
        for shop_count in &self.mode_config.shop_counts {
            for _ in 0..shop_count.count {
                let pos = crate::spawning::find_spawn_pos(&game);
                let shop_config = self.shop_configs.get(&shop_count.shop_id).unwrap();
                game.shops.push(Shop {
                    position: pos,
                    uses_remaining: shop_config.default_uses,
                    shop_id: shop_count.shop_id.clone(),
                });
            }
        }
    }
}
