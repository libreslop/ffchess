use common::*;
use std::collections::HashMap;
use uuid::Uuid;
use glam::IVec2;
use tokio::sync::RwLock;
use rand::Rng;

pub struct ServerState {
    pub game: RwLock<GameState>,
    pub player_channels: RwLock<HashMap<Uuid, tokio::sync::mpsc::UnboundedSender<ServerMessage>>>,
    pub removed_pieces: RwLock<Vec<Uuid>>,
    pub removed_players: RwLock<Vec<Uuid>>,
    // Player ID -> Hex Color
    pub player_colors: RwLock<HashMap<Uuid, String>>,
    // Color -> Last Active timestamp
    pub color_last_active: RwLock<HashMap<String, i64>>,
    pub last_viewed_at: RwLock<i64>,
}

const PREFERRED_COLORS: &[&str] = &[
    "#2563eb", // Blue
    "#dc2626", // Red
    "#16a34a", // Green
    "#d97706", // Orange
    "#9333ea", // Purple
    "#0891b2", // Cyan
    "#db2777", // Pink
    "#ca8a04", // Yellow
    "#4d7c0f", // Lime
    "#b91c1c", // Dark Red
    "#1d4ed8", // Dark Blue
    "#15803d", // Dark Green
    "#ea580c", // Dark Orange
];

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
            player_colors: RwLock::new(HashMap::new()),
            color_last_active: RwLock::new(HashMap::new()),
            last_viewed_at: RwLock::new(chrono::Utc::now().timestamp_millis()),
        }
    }

    pub fn calculate_board_size(player_count: usize) -> i32 {
        // (starts at 40x40, scales with player count using a square-root formula up to 200x200).
        // For the first 3 players, stay at 40.
        if player_count <= 3 {
            return 40;
        }
        // Hit ~200 at 100 players: 25 + sqrt(100) * 17.5 = 200
        // We keep the original scaling but clamp to 40.
        (25.0 + (player_count as f32).sqrt() * 17.5).clamp(40.0, 200.0) as i32
    }

    async fn get_or_assign_color(&self, player_id: Uuid) -> String {
        {
            let colors = self.player_colors.read().await;
            if let Some(color) = colors.get(&player_id) {
                return color.clone();
            }
        }

        let mut player_colors = self.player_colors.write().await;
        let mut color_last_active = self.color_last_active.write().await;
        let now = chrono::Utc::now().timestamp();

        // 1. Try to find a color that is not currently assigned to any ACTIVE player
        // We consider active players as those who have an entry in player_channels
        let active_player_ids: Vec<Uuid> = self.player_channels.read().await.keys().cloned().collect();
        let active_colors: Vec<String> = active_player_ids.iter().filter_map(|id| player_colors.get(id).cloned()).collect();

        for &c in PREFERRED_COLORS {
            let color = c.to_string();
            if !active_colors.contains(&color) {
                player_colors.insert(player_id, color.clone());
                color_last_active.insert(color.clone(), now);
                return color;
            }
        }

        // 2. If all preferred colors are used, try to find an expired one
        // Expire after 5 minutes
        for &c in PREFERRED_COLORS {
            let color = c.to_string();
            if let Some(&last_active) = color_last_active.get(&color) {
                if now - last_active > 300 {
                    player_colors.insert(player_id, color.clone());
                    color_last_active.insert(color.clone(), now);
                    return color;
                }
            }
        }

        // 3. Fallback: generate a random color
        let mut rng = rand::thread_rng();
        let color = format!("#{:06x}", rng.gen_range(0..0x1000000));
        player_colors.insert(player_id, color.clone());
        color_last_active.insert(color.clone(), now);
        color
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
        let color = self.get_or_assign_color(player_id).await;
        
        self.player_channels.write().await.insert(player_id, tx);

        let mut game = self.game.write().await;
        
        let new_size = Self::calculate_board_size(game.players.len() + 1);
        if new_size > game.board_size {
            game.board_size = new_size;
            self.prune_out_of_bounds(&mut game).await;
        }

        let spawn_pos = self.find_spawn_pos(&game).await;
        
        let mut rp = self.removed_pieces.write().await;
        game.pieces.retain(|id, p| {
            if p.owner_id.is_none() && (p.position - spawn_pos).as_vec2().length() <= 15.0 {
                rp.push(*id);
                false
            } else {
                true
            }
        });
        drop(rp);

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
            
            // Scatter randomly within 2 squares of king
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
            
            // Fallback: pick any neighbor within board boundaries
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

    async fn find_spawn_pos(&self, game: &GameState) -> IVec2 {
        let mut rng = rand::thread_rng();
        let board_size = game.board_size;
        let half = board_size / 2;
        let limit = (board_size + 1) / 2;
        
        // Ensure spawn is within board with a margin for kit pieces
        let margin = 3;

        for _ in 0..100 {
            let pos = IVec2::new(
                rng.gen_range(-half + margin..limit - margin), 
                rng.gen_range(-half + margin..limit - margin)
            );
            let mut occupied = false;
            
            // Check pieces
            for piece in game.pieces.values() {
                if (piece.position - pos).as_vec2().length() < 10.0 {
                    occupied = true;
                    break;
                }
            }
            
            if !occupied {
                // Check shops
                for shop in &game.shops {
                    if (shop.position - pos).as_vec2().length() < 5.0 {
                        occupied = true;
                        break;
                    }
                }
            }

            if !occupied {
                return pos;
            }
        }
        
        // Final Fallback: try to find ANYTHING not on a shop or piece
        for _ in 0..100 {
            let pos = IVec2::new(
                rng.gen_range(-half + margin..limit - margin), 
                rng.gen_range(-half + margin..limit - margin)
            );
            if !game.pieces.values().any(|p| p.position == pos) && !game.shops.iter().any(|s| s.position == pos) {
                return pos;
            }
        }

        IVec2::new(
            rng.gen_range(-half + margin..limit - margin), 
            rng.gen_range(-half + margin..limit - margin)
        )
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

        // Send Game Over message to this specific player before removing them
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

    pub async fn handle_move(&self, player_id: Uuid, piece_id: Uuid, target: IVec2) -> Result<(), String> {
        let mut game = self.game.write().await;
        let board_size = game.board_size;
        
        let now = chrono::Utc::now().timestamp_millis();
        
        let (start_pos, piece_type) = {
            let piece = game.pieces.get(&piece_id).ok_or("Piece not found")?;
            if piece.owner_id != Some(player_id) {
                return Err("Not your piece".to_string());
            }
            // Add 100ms grace period for latency/clock skew
            if now < piece.last_move_time + piece.cooldown_ms - 100 {
                return Err("On cooldown".to_string());
            }
            (piece.position, piece.piece_type)
        };

        let target_piece = game.pieces.values().find(|p| p.position == target);
        let is_capture = if let Some(tp) = target_piece {
            if tp.owner_id == Some(player_id) {
                return Err("Target occupied by friendly".to_string());
            }
            true
        } else {
            false
        };

        if !is_valid_chess_move(piece_type, start_pos, target, is_capture, board_size) {
            return Err("Invalid chess move".to_string());
        }

        if piece_type != PieceType::Knight && is_move_blocked(start_pos, target, &game.pieces) {
            return Err("Path is blocked".to_string());
        }

        let mut captured_player_id = None;
        if let Some(tp) = target_piece {
            let captured_id = tp.id;
            let captured_type = tp.piece_type;
            let value = get_piece_value(captured_type);
            
            if captured_type == PieceType::King {
                captured_player_id = tp.owner_id;
            }
            
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
            if let Some(p) = game.players.get_mut(&player_id) {
                p.kills += 1;
            }

            // Send GameOver to victim immediately
            if let Some((score, kills, pieces_captured, join_time)) = victim_stats {
                let now = chrono::Utc::now().timestamp();
                let duration = (now - join_time).max(0) as u64;
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
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerState {
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
        
        {
            let mut game = self.game.write().await;
            let target_size = Self::calculate_board_size(game.players.len());
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

        // Broadcast periodic state updates
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
