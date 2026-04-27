//! Shared game instance accessors.

use super::ServerState;
use crate::config::NamePool;
use crate::instance::GameInstance;
use common::protocol::ServerMessage;
use common::types::{ModeId, PlayerId, SessionSecret};
use std::sync::Arc;

impl ServerState {
    /// Returns the name pool used for auto-generated display names.
    pub fn name_pool(&self) -> &NamePool {
        &self.config_manager.name_pool
    }

    /// Returns the maximum allowed chat message length in characters.
    pub fn chat_message_max_chars(&self) -> usize {
        self.config_manager.global.chat_message_max_chars.max(1) as usize
    }

    /// Returns the game instance for a given mode id if present.
    pub(super) async fn get_game(&self, mode_id: &ModeId) -> Option<Arc<GameInstance>> {
        self.games.read().await.get(mode_id).cloned()
    }

    /// Returns a joinable public game instance for the given mode id.
    ///
    /// Hidden/private match worlds are never returned by this method.
    pub async fn get_joinable_game(&self, mode_id: &ModeId) -> Option<Arc<GameInstance>> {
        if self.private_game_ids.read().await.contains(mode_id) {
            return None;
        }
        self.get_game(mode_id).await
    }

    /// Sends an Init snapshot to a client with the public-mode identity.
    pub async fn send_init(
        &self,
        tx: &tokio::sync::mpsc::Sender<ServerMessage>,
        instance: &Arc<GameInstance>,
        player_id: PlayerId,
        session_secret: SessionSecret,
    ) {
        let mode = self
            .config_manager
            .modes
            .get(instance.public_mode_id())
            .map(|mode| mode.to_client_config())
            .unwrap_or_else(|| instance.client_mode_config());
        let mut state = instance.game.read().await.clone();
        let move_unlock_at = instance.move_unlock_at().await;
        let chat_room_key = instance.chat_room_key();
        let chat_history = instance.chat_history_snapshot().await;
        state.mode_id = instance.public_mode_id().clone();
        let _ = tx.try_send(ServerMessage::Init {
            player_id,
            session_secret,
            move_unlock_at,
            state: Box::new(state),
            mode: Box::new(mode),
            pieces: instance.piece_config_snapshot(),
            shops: instance.shop_config_snapshot(),
            chat_room_key,
            chat_history,
            sync_interval_ms: self.config_manager.global.sync_interval_ms,
        });
    }

    /// Finds the currently viewed instance for an unbound connection in a public mode.
    pub async fn watched_instance_for_connection(
        &self,
        mode_id: &ModeId,
        conn_id: crate::types::ConnectionId,
    ) -> Option<Arc<GameInstance>> {
        let instances = {
            let games = self.games.read().await;
            games
                .values()
                .filter(|instance| instance.public_mode_id() == mode_id)
                .cloned()
                .collect::<Vec<_>>()
        };
        for instance in instances {
            if instance.has_connection_channel(conn_id).await {
                return Some(instance);
            }
        }
        None
    }

    /// Returns all public (non-private) game instances.
    pub async fn public_game_instances(&self) -> Vec<(ModeId, Arc<GameInstance>)> {
        let hidden = self.private_game_ids.read().await.clone();
        let games = self.games.read().await;
        games
            .iter()
            .filter(|(mode_id, _)| !hidden.contains(*mode_id))
            .map(|(mode_id, instance)| (mode_id.clone(), instance.clone()))
            .collect()
    }

    /// Returns all private match instances.
    pub async fn private_game_instances(&self) -> Vec<(ModeId, Arc<GameInstance>)> {
        let hidden = self.private_game_ids.read().await.clone();
        let games = self.games.read().await;
        hidden
            .iter()
            .filter_map(|id| {
                games
                    .get(id)
                    .cloned()
                    .map(|instance| (id.clone(), instance))
            })
            .collect()
    }

    /// Initializes shops for all game instances.
    pub async fn spawn_initial_shops(&self) {
        let instances = {
            let games = self.games.read().await;
            games.values().cloned().collect::<Vec<_>>()
        };
        for instance in instances {
            instance.spawn_initial_shops().await;
        }
    }

    /// Ticks all game instances once.
    pub async fn tick_all_games(&self) {
        let instances = {
            let games = self.games.read().await;
            games.values().cloned().collect::<Vec<_>>()
        };
        for instance in instances {
            instance.handle_tick().await;
        }
    }

    /// Removes completed hidden match worlds that no longer have participants.
    pub async fn cleanup_private_games(&self) {
        let hidden_ids: Vec<ModeId> = self.private_game_ids.read().await.iter().cloned().collect();
        if hidden_ids.is_empty() {
            return;
        }

        let hidden_instances: Vec<(ModeId, Arc<GameInstance>)> = {
            let games = self.games.read().await;
            hidden_ids
                .iter()
                .filter_map(|id| games.get(id).cloned().map(|g| (id.clone(), g)))
                .collect()
        };

        let mut to_remove = Vec::new();
        for (id, instance) in hidden_instances {
            if instance.is_empty().await {
                to_remove.push(id);
            }
        }

        if to_remove.is_empty() {
            return;
        }

        let mut games = self.games.write().await;
        let mut hidden = self.private_game_ids.write().await;
        for id in to_remove {
            games.remove(&id);
            hidden.remove(&id);
        }
    }
}
