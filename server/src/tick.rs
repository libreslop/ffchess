use crate::state::ServerState;
use common::*;

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
            let target_size = calculate_board_size(game.players.len());
            if target_size < game.board_size {
                let any_player_pieces_outside = game
                    .pieces
                    .values()
                    .any(|p| p.owner_id.is_some() && !is_within_board(p.position, target_size));

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

    pub async fn prune_out_of_bounds(&self, game: &mut GameState) {
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
        game.shops
            .retain(|s| is_within_board(s.position, board_size));
    }
}
