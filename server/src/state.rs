//! Shared server state and mode instance registry.

use crate::config::ConfigManager;
use crate::instance::GameInstance;
use crate::types::ConnectionId;
use common::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

/// Active game binding for a websocket connection.
#[derive(Clone)]
pub struct ActivePlayerBinding {
    pub player_id: PlayerId,
    pub instance: Arc<GameInstance>,
}

/// Queue entry for matchmaking modes.
#[derive(Clone)]
pub struct MatchQueueEntry {
    pub conn_id: ConnectionId,
    pub tx: mpsc::UnboundedSender<common::protocol::ServerMessage>,
    pub name: String,
    pub kit_name: KitId,
}

/// Result of attempting to enqueue a player into a matchmaking queue.
pub enum QueueJoinResult {
    Waiting,
    Matched {
        match_instance: Arc<GameInstance>,
        players: Vec<MatchQueueEntry>,
    },
}

/// Shared server state containing mode instances and configuration.
pub struct ServerState {
    pub games: Arc<RwLock<HashMap<ModeId, Arc<GameInstance>>>>,
    pub config_manager: Arc<ConfigManager>,
    pub piece_configs: Arc<HashMap<PieceTypeId, PieceConfig>>,
    pub shop_configs: Arc<HashMap<ShopId, ShopConfig>>,
    pub private_game_ids: Arc<RwLock<HashSet<ModeId>>>,
    pub queue_entries: Arc<RwLock<HashMap<ModeId, Vec<MatchQueueEntry>>>>,
    pub connection_bindings: Arc<RwLock<HashMap<ConnectionId, ActivePlayerBinding>>>,
}

impl ServerState {
    /// Builds the global server state and loads configuration.
    ///
    /// Returns a `ServerState` with one `GameInstance` per configured mode.
    pub fn new() -> Self {
        let config_manager = Arc::new(ConfigManager::load(std::path::Path::new("config")));
        let mut games = HashMap::new();

        let piece_configs = Arc::new(config_manager.pieces.clone());
        let shop_configs = Arc::new(config_manager.shops.clone());

        for (mode_id, mode_config) in &config_manager.modes {
            let instance = Arc::new(GameInstance::new(
                mode_config.clone(),
                piece_configs.clone(),
                shop_configs.clone(),
            ));
            games.insert(mode_id.clone(), instance);
        }

        Self {
            games: Arc::new(RwLock::new(games)),
            config_manager,
            piece_configs,
            shop_configs,
            private_game_ids: Arc::new(RwLock::new(HashSet::new())),
            queue_entries: Arc::new(RwLock::new(HashMap::new())),
            connection_bindings: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns the game instance for a given mode id if present.
    ///
    /// `mode_id` identifies the desired game mode. Returns an `Arc<GameInstance>` if found.
    pub async fn get_game(&self, mode_id: &ModeId) -> Option<Arc<GameInstance>> {
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

    /// Returns the configured queue size for a mode if it is a matchmaking mode.
    pub fn queue_target_players(&self, mode_id: &ModeId) -> Option<u32> {
        self.config_manager
            .modes
            .get(mode_id)
            .and_then(|m| (m.queue_players >= 2).then_some(m.queue_players))
    }

    /// Returns the current queue length for a mode.
    pub async fn queue_len(&self, mode_id: &ModeId) -> u32 {
        self.queue_entries
            .read()
            .await
            .get(mode_id)
            .map(|q| q.len() as u32)
            .unwrap_or(0)
    }

    /// Returns a snapshot of current queue entries and required size.
    pub async fn queue_snapshot(&self, mode_id: &ModeId) -> Option<(u32, Vec<MatchQueueEntry>)> {
        let required = self.queue_target_players(mode_id)?;
        let entries = self
            .queue_entries
            .read()
            .await
            .get(mode_id)
            .cloned()
            .unwrap_or_default();
        Some((required, entries))
    }

    /// Removes a connection from a mode queue if present.
    pub async fn remove_from_queue(&self, mode_id: &ModeId, conn_id: ConnectionId) -> bool {
        let mut queues = self.queue_entries.write().await;
        let (removed, became_empty) = {
            let Some(entries) = queues.get_mut(mode_id) else {
                return false;
            };
            let before = entries.len();
            entries.retain(|e| e.conn_id != conn_id);
            (entries.len() != before, entries.is_empty())
        };
        if became_empty {
            queues.remove(mode_id);
        }
        removed
    }

    /// Enqueues a player for a matchmaking mode.
    ///
    /// Returns `None` if the mode is not a matchmaking mode.
    pub async fn enqueue_matchmaking(
        &self,
        mode_id: &ModeId,
        entry: MatchQueueEntry,
    ) -> Option<QueueJoinResult> {
        let required = self.queue_target_players(mode_id)? as usize;
        let players = {
            let mut queues = self.queue_entries.write().await;
            let queue = queues.entry(mode_id.clone()).or_default();
            if let Some(idx) = queue.iter().position(|e| e.conn_id == entry.conn_id) {
                queue.remove(idx);
            }
            queue.push(entry);
            if queue.len() < required {
                return Some(QueueJoinResult::Waiting);
            }
            queue.drain(0..required).collect::<Vec<_>>()
        };

        let template = self.config_manager.modes.get(mode_id)?.clone();
        let private_mode_id = ModeId::from(format!("{}__{}", mode_id.as_ref(), Uuid::new_v4()));
        let mut private_mode = template;
        private_mode.id = private_mode_id.clone();
        private_mode.queue_players = 0;

        let match_instance = Arc::new(GameInstance::new(
            private_mode,
            self.piece_configs.clone(),
            self.shop_configs.clone(),
        ));
        match_instance.spawn_initial_shops().await;

        self.games
            .write()
            .await
            .insert(private_mode_id.clone(), match_instance.clone());
        self.private_game_ids.write().await.insert(private_mode_id);

        Some(QueueJoinResult::Matched {
            match_instance,
            players,
        })
    }

    /// Stores an active connection->player binding.
    pub async fn bind_connection(
        &self,
        conn_id: ConnectionId,
        player_id: PlayerId,
        instance: Arc<GameInstance>,
    ) {
        self.connection_bindings.write().await.insert(
            conn_id,
            ActivePlayerBinding {
                player_id,
                instance,
            },
        );
    }

    /// Looks up an active connection->player binding.
    pub async fn get_binding(&self, conn_id: ConnectionId) -> Option<ActivePlayerBinding> {
        self.connection_bindings.read().await.get(&conn_id).cloned()
    }

    /// Removes and returns an active connection->player binding.
    pub async fn unbind_connection(&self, conn_id: ConnectionId) -> Option<ActivePlayerBinding> {
        self.connection_bindings.write().await.remove(&conn_id)
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
            let has_players = !instance.game.read().await.players.is_empty();
            let has_player_channels = !instance.player_channels.read().await.is_empty();
            let has_viewers = !instance.connection_channels.read().await.is_empty();
            if !has_players && !has_player_channels && !has_viewers {
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

impl Default for ServerState {
    /// Provides a default server state by loading configuration.
    fn default() -> Self {
        Self::new()
    }
}
