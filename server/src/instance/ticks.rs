//! Periodic tick processing for game instances.

use super::GameInstance;
use crate::time::now_ms;
use common::protocol::ServerMessage;
use common::types::DurationMs;

impl GameInstance {
    /// Runs a single server tick: updates timers, broadcasts state, and cleans up.
    ///
    /// Returns nothing; this mutates game state and sends updates.
    pub async fn handle_tick(&self) {
        self.start_tick_hooks().await;
        let now = now_ms();
        let players_viewing = !self.player_channels.read().await.is_empty()
            || !self.connection_channels.read().await.is_empty();

        if players_viewing {
            *self.last_viewed_at.write().await = now;
        }

        let last_viewed = *self.last_viewed_at.read().await;
        if now - last_viewed < DurationMs::from_millis(5000) {
            self.process_queued_moves().await;
            self.spawn_npcs().await;
            self.tick_npcs().await;
        }

        // Periodic cleanup (approx. every minute)
        static TICK_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tick = TICK_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if tick.is_multiple_of(600) {
            // Cleanup death timestamps
            let mut dt = self.death_timestamps.write().await;
            dt.retain(|_, timestamp_ms| {
                now - *timestamp_ms <= DurationMs::from_millis(10 * 60 * 1000)
            });

            let mut cm = self.color_manager.write().await;
            cm.cleanup(now.as_i64() / 1000, 24 * 60 * 60);
        }

        {
            let mut cm = self.color_manager.write().await;
            let channels = self.player_channels.read().await;
            for player_id in channels.keys() {
                cm.update_activity(*player_id);
            }
        }

        {
            let mut game = self.game.write().await;
            let target_size =
                common::logic::calculate_board_size(&self.mode_config, game.players.len());
            if target_size < game.board_size {
                let any_player_pieces_outside = game.pieces.values().any(|p| {
                    p.owner_id.is_some() && !common::logic::is_within_board(p.position, target_size)
                });

                if !any_player_pieces_outside {
                    game.board_size = target_size;
                    self.prune_out_of_bounds(&mut game).await;
                }
            }
        }

        self.resolve_tick_hooks().await;

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
}
