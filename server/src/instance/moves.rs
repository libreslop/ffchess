//! Move execution logic for a game instance.

use super::{GameInstance, game_instance::QueuedMoveRequest};
use crate::time::now_ms;
use common::models::GameState;
use common::protocol::{GameError, ServerMessage};
use common::types::{BoardCoord, PieceId, PlayerId};
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
        target: BoardCoord,
    ) -> Result<(), GameError> {
        let should_queue = {
            let game = self.game.read().await;
            let piece = game.pieces.get(&piece_id).ok_or(GameError::PieceNotFound)?;
            if piece.owner_id != Some(player_id) {
                return Err(GameError::NotYourPiece);
            }
            let elapsed = now_ms() - piece.last_move_time;
            elapsed < piece.cooldown_ms
        };

        if should_queue || self.piece_has_queued_moves(piece_id).await {
            self.queued_moves
                .write()
                .await
                .entry(piece_id)
                .or_default()
                .push_back(QueuedMoveRequest { player_id, target });
            return Ok(());
        }

        let mut game = self.game.write().await;
        self.apply_move_with_game(&mut game, player_id, piece_id, target, true)
            .await
    }

    /// Tries to execute server-queued premoves for pieces whose cooldown has elapsed.
    pub async fn process_queued_moves(&self) {
        let mut game = self.game.write().await;
        let mut queued_moves = self.queued_moves.write().await;
        let mut move_errors = Vec::<(PlayerId, GameError)>::new();
        let queued_piece_ids: Vec<_> = queued_moves.keys().copied().collect();

        for piece_id in queued_piece_ids {
            loop {
                let is_ready = match game.pieces.get(&piece_id) {
                    Some(piece) => now_ms() - piece.last_move_time >= piece.cooldown_ms,
                    None => {
                        queued_moves.remove(&piece_id);
                        break;
                    }
                };
                if !is_ready {
                    break;
                }

                let next_request = match queued_moves
                    .get_mut(&piece_id)
                    .and_then(std::collections::VecDeque::pop_front)
                {
                    Some(request) => request,
                    None => break,
                };

                match self
                    .apply_move_with_game(
                        &mut game,
                        next_request.player_id,
                        piece_id,
                        next_request.target,
                        false,
                    )
                    .await
                {
                    Ok(()) => {}
                    Err(error) => move_errors.push((next_request.player_id, error)),
                }
            }
        }

        queued_moves.retain(|_, moves| !moves.is_empty());
        drop(queued_moves);
        drop(game);

        let player_channels = self.player_channels.read().await;
        for (player_id, error) in move_errors {
            if let Some(tx) = player_channels.get(&player_id) {
                let _ = tx.send(ServerMessage::Error(error));
            }
        }
    }

    /// Clears all server-queued premoves for a specific piece.
    pub async fn clear_queued_moves(&self, piece_id: PieceId) {
        let mut queued_moves = self.queued_moves.write().await;
        queued_moves.remove(&piece_id);
    }

    /// Returns true if a piece currently has queued server-side move requests.
    async fn piece_has_queued_moves(&self, piece_id: PieceId) -> bool {
        self.queued_moves
            .read()
            .await
            .get(&piece_id)
            .is_some_and(|moves| !moves.is_empty())
    }

    /// Validates and applies a single move using the provided mutable game state.
    async fn apply_move_with_game(
        &self,
        game: &mut GameState,
        player_id: PlayerId,
        piece_id: PieceId,
        target: BoardCoord,
        enforce_cooldown: bool,
    ) -> Result<(), GameError> {
        let (piece_type, start_pos, piece_owner) = {
            let piece = game.pieces.get(&piece_id).ok_or(GameError::PieceNotFound)?;
            if piece.owner_id != Some(player_id) {
                return Err(GameError::NotYourPiece);
            }

            if enforce_cooldown {
                let elapsed = now_ms() - piece.last_move_time;
                if elapsed < piece.cooldown_ms {
                    return Err(GameError::OnCooldown);
                }
            }

            (piece.piece_type.clone(), piece.position, piece.owner_id)
        };

        let target_piece = game
            .pieces
            .values()
            .find(|piece| piece.position == target)
            .cloned();
        let is_capture = if let Some(ref target_piece) = target_piece {
            if target_piece.owner_id == Some(player_id) {
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

        if let Some(target_piece) = target_piece {
            self.capture_piece(target_piece, Some(player_id), game)
                .await;
        }

        if let Some(piece) = game.pieces.get_mut(&piece_id) {
            piece.position = target;
            piece.last_move_time = now_ms();
            piece.cooldown_ms = piece_config.cooldown_ms;
            return Ok(());
        }

        Err(GameError::PieceNotFound)
    }
}
