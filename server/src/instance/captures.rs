//! Shared capture application for player and NPC moves.

use super::GameInstance;
use common::models::{GameState, Piece};
use common::types::{PlayerId, Score};
use glam::IVec2;

impl GameInstance {
    /// Removes the piece at `target` and records all capture side effects.
    pub(super) async fn capture_piece_at(
        &self,
        target: IVec2,
        capturer_id: Option<PlayerId>,
        game: &mut GameState,
    ) -> Option<Piece> {
        let captured_piece = game
            .pieces
            .values()
            .find(|piece| piece.position == target)
            .cloned()?;
        self.capture_piece(captured_piece, capturer_id, game).await
    }

    /// Removes `captured_piece` and records all capture side effects.
    pub(super) async fn capture_piece(
        &self,
        captured_piece: Piece,
        capturer_id: Option<PlayerId>,
        game: &mut GameState,
    ) -> Option<Piece> {
        let killed_event = if captured_piece.piece_type.is_king() {
            captured_piece.owner_id.and_then(|victim_id| {
                game.players.get(&victim_id).map(|victim| {
                    let killer_name = capturer_id
                        .and_then(|killer_id| game.players.get(&killer_id).map(|p| p.name.clone()));
                    (victim.name.clone(), killer_name)
                })
            })
        } else {
            None
        };

        game.pieces.remove(&captured_piece.id)?;
        self.record_piece_removal(captured_piece.id).await;

        if let Some(player_id) = capturer_id {
            self.apply_player_capture_rewards(player_id, &captured_piece, game);
        }

        self.record_capture_event(
            capturer_id,
            captured_piece.piece_type.clone(),
            captured_piece.owner_id,
            captured_piece.position,
        )
        .await;
        if let Some((victim_name, killer_name)) = killed_event {
            self.record_player_killed_event(victim_name, killer_name)
                .await;
        }

        Some(captured_piece)
    }

    /// Updates the attacker's score and counters for a successful capture.
    fn apply_player_capture_rewards(
        &self,
        player_id: PlayerId,
        captured_piece: &Piece,
        game: &mut GameState,
    ) {
        let capture_score = self
            .piece_configs
            .get(&captured_piece.piece_type)
            .map(|config| config.score_value)
            .unwrap_or_else(Score::zero);

        if let Some(player) = game.players.get_mut(&player_id) {
            player.score += capture_score;
            player.pieces_captured += 1;
            if captured_piece.piece_type.is_king() {
                player.kills += 1;
            }
        }
    }
}
