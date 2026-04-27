//! Move execution logic for a game instance.

use super::{GameInstance, game_instance::QueuedMoveRequest};
use crate::time::now_ms;
use common::models::{GameState, Piece};
use common::protocol::{GameError, ServerMessage};
use common::types::{BoardCoord, DurationMs, PieceId, PlayerId, TimestampMs};
use std::collections::VecDeque;

/// Fully validated move data ready to apply to either live or projected game state.
struct PreparedMove {
    cooldown_ms: DurationMs,
    captured_piece: Option<Piece>,
}

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
        let moves_locked = self
            .move_unlock_at()
            .await
            .is_some_and(|unlock_at| now_ms() < unlock_at);
        let should_queue = {
            let game = self.game.read().await;
            let piece = game.pieces.get(&piece_id).ok_or(GameError::PieceNotFound)?;
            if piece.owner_id != Some(player_id) {
                return Err(GameError::NotYourPiece);
            }
            let elapsed = now_ms() - piece.last_move_time;
            moves_locked || elapsed < piece.cooldown_ms
        };

        if should_queue || self.piece_has_queued_moves(piece_id).await {
            let game_snapshot = self.game.read().await.clone();
            let existing_queue = self
                .queued_moves
                .read()
                .await
                .get(&piece_id)
                .cloned()
                .unwrap_or_default();
            self.validate_queued_move_before_enqueue(
                &game_snapshot,
                piece_id,
                &existing_queue,
                player_id,
                target,
            )?;

            let mut queued_moves = self.queued_moves.write().await;
            let queue = queued_moves.entry(piece_id).or_default();
            if queue.len() < 100 {
                queue.push_back(QueuedMoveRequest { player_id, target });
            }
            return Ok(());
        }

        let mut game = self.game.write().await;
        self.apply_move_with_game(&mut game, player_id, piece_id, target, true)
            .await
    }

    /// Tries to execute server-queued premoves for pieces whose cooldown has elapsed.
    pub async fn process_queued_moves(&self) {
        if self
            .move_unlock_at()
            .await
            .is_some_and(|unlock_at| now_ms() < unlock_at)
        {
            return;
        }

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
                let _ = tx.try_send(ServerMessage::Error(error));
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
        let prepared = self.prepare_move(game, player_id, piece_id, target, enforce_cooldown)?;
        self.apply_prepared_live_move(game, player_id, piece_id, target, prepared)
            .await
    }

    /// Validates one move against the provided game state without mutating it.
    fn prepare_move(
        &self,
        game: &GameState,
        player_id: PlayerId,
        piece_id: PieceId,
        target: BoardCoord,
        enforce_cooldown: bool,
    ) -> Result<PreparedMove, GameError> {
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
            .find(|piece| piece.position == target && piece.id != piece_id)
            .cloned();
        let is_capture = target_piece.is_some();
        if target_piece
            .as_ref()
            .is_some_and(|piece| piece.owner_id == Some(player_id))
        {
            return Err(GameError::TargetFriendly);
        }

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

        Ok(PreparedMove {
            cooldown_ms: piece_config.cooldown_ms,
            captured_piece: target_piece,
        })
    }

    /// Applies one validated move to live game state, including capture side effects.
    async fn apply_prepared_live_move(
        &self,
        game: &mut GameState,
        player_id: PlayerId,
        piece_id: PieceId,
        target: BoardCoord,
        prepared: PreparedMove,
    ) -> Result<(), GameError> {
        if let Some(target_piece) = prepared.captured_piece {
            self.capture_piece(target_piece, Some(player_id), game)
                .await;
        }

        self.finish_prepared_move(
            game,
            player_id,
            piece_id,
            target,
            prepared.cooldown_ms,
            Some(now_ms()),
        )
    }

    fn validate_queued_move_before_enqueue(
        &self,
        base_game: &GameState,
        piece_id: PieceId,
        existing_queue: &VecDeque<QueuedMoveRequest>,
        player_id: PlayerId,
        target: BoardCoord,
    ) -> Result<(), GameError> {
        let mut projected = base_game.clone();
        for queued in existing_queue {
            self.validate_and_apply_projected_move(
                &mut projected,
                piece_id,
                queued.player_id,
                queued.target,
            )?;
        }
        self.validate_and_apply_projected_move(&mut projected, piece_id, player_id, target)
    }

    fn validate_and_apply_projected_move(
        &self,
        game: &mut GameState,
        piece_id: PieceId,
        player_id: PlayerId,
        target: BoardCoord,
    ) -> Result<(), GameError> {
        let prepared = self.prepare_move(game, player_id, piece_id, target, false)?;
        self.apply_prepared_projected_move(game, player_id, piece_id, target, prepared)
    }

    /// Applies one validated move to projected state used for queued premove validation.
    fn apply_prepared_projected_move(
        &self,
        game: &mut GameState,
        player_id: PlayerId,
        piece_id: PieceId,
        target: BoardCoord,
        prepared: PreparedMove,
    ) -> Result<(), GameError> {
        if let Some(target_piece) = prepared.captured_piece {
            game.pieces.remove(&target_piece.id);
        }

        self.finish_prepared_move(
            game,
            player_id,
            piece_id,
            target,
            prepared.cooldown_ms,
            None,
        )
    }

    /// Updates the moved piece after validation and capture handling succeed.
    fn finish_prepared_move(
        &self,
        game: &mut GameState,
        player_id: PlayerId,
        piece_id: PieceId,
        target: BoardCoord,
        cooldown_ms: DurationMs,
        last_move_time: Option<TimestampMs>,
    ) -> Result<(), GameError> {
        if let Some(piece) = game.pieces.get_mut(&piece_id) {
            piece.position = target;
            if let Some(last_move_time) = last_move_time {
                piece.last_move_time = last_move_time;
            }
            piece.cooldown_ms = cooldown_ms;
        } else {
            return Err(GameError::PieceNotFound);
        }

        self.try_auto_upgrade_single_item(game, player_id, target);
        Ok(())
    }
}
