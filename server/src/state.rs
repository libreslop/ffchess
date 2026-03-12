use common::*;
use std::collections::HashMap;
use uuid::Uuid;
use glam::IVec2;
use tokio::sync::RwLock;
use rand::Rng;
use crate::colors::ColorManager;
use crate::spawning::find_spawn_pos;

pub struct ServerState {
    pub game: RwLock<GameState>,
    pub player_channels: RwLock<HashMap<Uuid, tokio::sync::mpsc::UnboundedSender<ServerMessage>>>,
    pub removed_pieces: RwLock<Vec<Uuid>>,
    pub removed_players: RwLock<Vec<Uuid>>,
    pub color_manager: RwLock<ColorManager>,
    pub last_viewed_at: RwLock<i64>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            game: RwLock::new(GameState {
                board_size: 40,
                ..Default::default()
            }),
            player_channels: RwLock::new(HashMap::new()),
            removed_pieces: RwLock::new(Vec::new()),
            removed_players: RwLock::new(Vec::new()),
            color_manager: RwLock::new(ColorManager::new()),
            last_viewed_at: RwLock::new(chrono::Utc::now().timestamp_millis()),
        }
    }

    async fn prune_out_of_bounds(&self, game: &mut GameState) {
        let board_size = game.board_size;
        let mut rp = self.removed_pieces.write().await;
        game.pieces.retain(|id, p| {
            if !is_within_board(p.position, board_size) {
                rp.push(*id);
                false
            } else {
                true
            }
        });
        game.shops.retain(|s| is_within_board(s.position, board_size));
    }

    pub async fn add_player(&self, name: String, kit: KitType, tx: tokio::sync::mpsc::UnboundedSender<ServerMessage>, existing_id: Option<Uuid>) -> Uuid {
        let player_id = existing_id.unwrap_or_else(Uuid::new_v4);
        
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
        game.pieces.insert(king_id, Piece {
            id: king_id,
            owner_id: Some(player_id),
            piece_type: PieceType::King,
            position: spawn_pos,
            last_move_time: 0,
            cooldown_ms: 0,
        });
        
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
                    && !game.shops.iter().any(|s| s.position == candidate) {
                    p_pos = candidate;
                    break;
                }
            }
            
            if p_pos == spawn_pos {
                let neighbors = [
                    IVec2::new(1, 0), IVec2::new(-1, 0), IVec2::new(0, 1), IVec2::new(0, -1),
                    IVec2::new(1, 1), IVec2::new(-1, 1), IVec2::new(1, -1), IVec2::new(-1, -1)
                ];
                for offset in neighbors {
                    let candidate = spawn_pos + offset;
                    if is_within_board(candidate, game.board_size) 
                        && !game.shops.iter().any(|s| s.position == candidate) {
                        p_pos = candidate;
                        break;
                    }
                }
            }

            game.pieces.insert(p_id, Piece {
                id: p_id,
                owner_id: Some(player_id),
                piece_type: p_type,
                position: p_pos,
                last_move_time: 0,
                cooldown_ms: 0,
            });
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
        player_id
    }

    pub async fn record_piece_removal(&self, piece_id: Uuid) {
        self.removed_pieces.write().await.push(piece_id);
    }

    pub async fn record_player_removal(&self, player_id: Uuid, game: &mut GameState) {
        self.removed_players.write().await.push(player_id);
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

    pub async fn remove_player(&self, player_id: Uuid) {
        let stats = {
            let game = self.game.read().await;
            game.players.get(&player_id).map(|p| (p.score, p.kills, p.pieces_captured, p.join_time))
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
        self.record_player_removal(player_id, &mut game).await;
    }

    pub async fn handle_move(&self, player_id: Uuid, piece_id: Uuid, target: IVec2) -> Result<(), GameError> {
        let mut game = self.game.write().await;
        let board_size = game.board_size;
        let now = chrono::Utc::now().timestamp_millis();
        
        let (start_pos, piece_type) = {
            let piece = game.pieces.get(&piece_id).ok_or(GameError::PieceNotFound)?;
            if piece.owner_id != Some(player_id) { return Err(GameError::NotYourPiece); }
            if now < piece.last_move_time + piece.cooldown_ms - 100 { return Err(GameError::OnCooldown); }
            (piece.position, piece.piece_type)
        };

        let target_piece = game.pieces.values().find(|p| p.position == target);
        let is_capture = if let Some(tp) = target_piece {
            if tp.owner_id == Some(player_id) { return Err(GameError::TargetFriendly); }
            true
        } else {
            false
        };

        if !is_valid_chess_move(piece_type, start_pos, target, is_capture, board_size) {
            return Err(GameError::InvalidMove);
        }

        if piece_type != PieceType::Knight && is_move_blocked(start_pos, target, &game.pieces) {
            return Err(GameError::PathBlocked);
        }

        let mut captured_player_id = None;
        if let Some(tp) = target_piece {
            let captured_id = tp.id;
            let captured_type = tp.piece_type;
            let value = get_piece_value(captured_type);
            
            if captured_type == PieceType::King { captured_player_id = tp.owner_id; }
            
            game.pieces.remove(&captured_id);
            self.record_piece_removal(captured_id).await;
            if let Some(p) = game.players.get_mut(&player_id) {
                p.score += value;
                p.pieces_captured += 1;
            }
        }

        let config = game.cooldown_config.clone();
        if let Some(p) = game.pieces.get_mut(&piece_id) {
            p.position = target;
            p.last_move_time = now;
            p.cooldown_ms = calculate_cooldown(piece_type, start_pos, target, &config);
        }

        if let Some(cp_id) = captured_player_id {
            let victim_stats = game.players.get(&cp_id).map(|p| (p.score, p.kills, p.pieces_captured, p.join_time));
            game.players.remove(&cp_id);
            self.record_player_removal(cp_id, &mut game).await;
            if let Some(p) = game.players.get_mut(&player_id) { p.kills += 1; }

            if let Some((score, kills, pieces_captured, join_time)) = victim_stats {
                let duration = (chrono::Utc::now().timestamp() - join_time).max(0) as u64;
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

        Ok(())
    }

    pub async fn broadcast(&self, msg: ServerMessage) {
        let channels = self.player_channels.read().await;
        for tx in channels.values() {
            let _ = tx.send(msg.clone());
        }
    }

    pub async fn handle_tick(&self) {
        let now = chrono::Utc::now().timestamp_millis();
        let players_viewing = !self.player_channels.read().await.is_empty();
        
        if players_viewing { *self.last_viewed_at.write().await = now; }

        let last_viewed = *self.last_viewed_at.read().await;
        if now - last_viewed < 5000 { self.tick_npcs().await; }
        
        {
            let mut game = self.game.write().await;
            let target_size = calculate_board_size(game.players.len());
            if target_size < game.board_size {
                let any_player_pieces_outside = game.pieces.values().any(|p| {
                    p.owner_id.is_some() && !is_within_board(p.position, target_size)
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
        }).await;
    }
}

impl Default for ServerState {
    fn default() -> Self { Self::new() }
}
