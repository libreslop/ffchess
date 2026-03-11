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
}

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
        }
    }

    pub fn calculate_board_size(player_count: usize) -> i32 {
        (30 + (player_count as i32 * 10)).min(200)
    }

    pub async fn add_player(&self, name: String, kit: KitType, tx: tokio::sync::mpsc::UnboundedSender<ServerMessage>) -> Uuid {
        let player_id = Uuid::new_v4();
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
        
        for (i, p_type) in kit.get_pieces().into_iter().enumerate() {
            let p_id = Uuid::new_v4();
            let offset = IVec2::new((i as i32 % 3) - 1, (i as i32 / 3) + 1);
            let mut p_pos = spawn_pos + offset;
            p_pos.x = p_pos.x.clamp(0, game.board_size - 1);
            p_pos.y = p_pos.y.clamp(0, game.board_size - 1);

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
