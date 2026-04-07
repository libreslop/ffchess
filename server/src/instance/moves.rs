//! Move execution logic for a game instance.

use super::GameInstance;
use crate::time::now_ms;
use common::protocol::GameError;
use common::types::{PieceId, PlayerId, Score};
use glam::IVec2;

impl GameInstance {
    /// Validates and applies a move for a player's piece.
    ///
    /// `player_id` is the moving player, `piece_id` selects the piece, and `target` is the tile.
    /// Returns `Ok(())` on success or a `GameError` on failure.
    pub async fn handle_move(
        &self,
        player_id: PlayerId,
        piece_id: PieceId,
        target: IVec2,
    ) -> Result<(), GameError> {
        let mut game = self.game.write().await;

        let (piece_type, start_pos, piece_owner) = {
            let piece = game.pieces.get(&piece_id).ok_or(GameError::PieceNotFound)?;
            if piece.owner_id != Some(player_id) {
                return Err(GameError::NotYourPiece);
            }
            let now = now_ms();
            let elapsed = now - piece.last_move_time;
            if elapsed < piece.cooldown_ms {
                return Err(GameError::OnCooldown);
            }
            (piece.piece_type.clone(), piece.position, piece.owner_id)
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

        if !common::logic::is_valid_move(common::logic::MoveValidationParams {
            piece_config,
            start: start_pos,
            end: target,
            is_capture,
            board_size: game.board_size,
            pieces: &game.pieces,
            moving_owner: piece_owner,
        }) {
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
                .unwrap_or_else(Score::zero);

            if let Some(player) = game.players.get_mut(&player_id) {
                player.score += attacker_score;
                player.pieces_captured += 1;
                if tp.piece_type.is_king() {
                    player.kills += 1;
                }
            }

            self.record_capture_event(Some(player_id), tp.piece_type.clone(), tp.owner_id)
                .await;
        }

        if let Some(piece) = game.pieces.get_mut(&piece_id) {
            piece.position = target;
            piece.last_move_time = now_ms();

            // Cooldown uses the base value from config for now.
            piece.cooldown_ms = piece_config.cooldown_ms;
        }

        Ok(())
    }
}
