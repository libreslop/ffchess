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
                board_size: 30,
                ..Default::default()
            }),
            player_channels: RwLock::new(HashMap::new()),
            removed_pieces: RwLock::new(Vec::new()),
            removed_players: RwLock::new(Vec::new()),
            player_colors: RwLock::new(HashMap::new()),
            color_last_active: RwLock::new(HashMap::new()),
        }
    }

    pub fn calculate_board_size(player_count: usize) -> i32 {
        // Diminishing returns: 30 + sqrt(count) * 25
        (30 + ((player_count as f32).sqrt() * 25.0) as i32).min(200)
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

    pub async fn add_player(&self, name: String, kit: KitType, tx: tokio::sync::mpsc::UnboundedSender<ServerMessage>, existing_id: Option<Uuid>) -> Uuid {
        let player_id = existing_id.unwrap_or_else(Uuid::new_v4);
        let color = self.get_or_assign_color(player_id).await;
        
        self.player_channels.write().await.insert(player_id, tx);

        let mut game = self.game.write().await;
        
        game.board_size = Self::calculate_board_size(game.players.len() + 1);

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
                    && !game.pieces.values().any(|p| p.position == candidate) {
                    p_pos = candidate;
                    break;
                }
            }
            
            // Fallback: if we couldn't find an empty spot, just pick any valid neighbor
            if p_pos == spawn_pos {
                let offset = IVec2::new(rng.gen_range(-1..=1), rng.gen_range(-1..=1));
                p_pos = (spawn_pos + offset).clamp(IVec2::ZERO, IVec2::new(game.board_size - 1, game.board_size - 1));
                if p_pos == spawn_pos { p_pos.x = (p_pos.x + 1).min(game.board_size - 1); }
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
            king_id,
            color,
        };
        
        game.players.insert(player_id, player);
        
        player_id
    }

    async fn find_spawn_pos(&self, game: &GameState) -> IVec2 {
        let mut rng = rand::thread_rng();
        let board_size = game.board_size;
        for _ in 0..100 {
            let pos = IVec2::new(rng.gen_range(0..board_size), rng.gen_range(0..board_size));
            let mut occupied = false;
            for piece in game.pieces.values() {
                if (piece.position - pos).as_vec2().length() < 10.0 {
                    occupied = true;
                    break;
                }
            }
            if !occupied {
                return pos;
            }
        }
        // Fallback: just a random position if we can't find an empty one
        IVec2::new(rng.gen_range(0..board_size), rng.gen_range(0..board_size))
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
        let score = {
            let game = self.game.read().await;
            game.players.get(&player_id).map(|p| p.score).unwrap_or(0)
        };

        // Send Game Over message to this specific player before removing them
        let channels = self.player_channels.read().await;
        if let Some(tx) = channels.get(&player_id) {
            let _ = tx.send(ServerMessage::GameOver { final_score: score });
        }
        drop(channels);

        let mut game = self.game.write().await;
        game.players.remove(&player_id);
        self.record_player_removal(player_id, &mut game).await;
        game.board_size = Self::calculate_board_size(game.players.len());
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
            }
        }

        if let Some(p) = game.pieces.get_mut(&piece_id) {
            p.position = target;
            p.last_move_time = now;
            p.cooldown_ms = calculate_cooldown(piece_type, start_pos, target);
        }

        if let Some(cp_id) = captured_player_id {
            game.players.remove(&cp_id);
            self.record_player_removal(cp_id, &mut game).await;
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
        self.tick_npcs().await;
        
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
        }).await;
    }
}
