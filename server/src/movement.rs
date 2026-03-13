use crate::state::ServerState;
use common::*;
use glam::IVec2;
use uuid::Uuid;

impl ServerState {
    pub async fn handle_move(
        &self,
        player_id: Uuid,
        piece_id: Uuid,
        target: IVec2,
    ) -> Result<(), GameError> {
        let mut game = self.game.write().await;
        let board_size = game.board_size;
        let now = chrono::Utc::now().timestamp_millis();

        let (start_pos, piece_type) = {
            let piece = game.pieces.get(&piece_id).ok_or(GameError::PieceNotFound)?;
            if piece.owner_id != Some(player_id) {
                return Err(GameError::NotYourPiece);
            }
            if now < piece.last_move_time + piece.cooldown_ms - 100 {
                return Err(GameError::OnCooldown);
            }
            (piece.position, piece.piece_type)
        };

        let target_piece = game.pieces.values().find(|p| p.position == target);
        let is_capture = if let Some(tp) = target_piece {
            if tp.owner_id == Some(player_id) {
                return Err(GameError::TargetFriendly);
            }
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
            let victim_stats = game
                .players
                .get(&cp_id)
                .map(|p| (p.score, p.kills, p.pieces_captured, p.join_time));
            game.players.remove(&cp_id);
            self.record_player_removal(cp_id, &mut game).await;
            if let Some(p) = game.players.get_mut(&player_id) {
                p.kills += 1;
            }

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

        Ok(())
    }
}
