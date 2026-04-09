//! Shared server state and mode instance registry.

use crate::config::ConfigManager;
use crate::instance::GameInstance;
use crate::time::now_ms;
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

/// Tracks preview watchers and the active preview target for a matchmaking mode.
struct ModePreviewState {
    target_mode_id: Option<ModeId>,
    empty_since: Option<TimestampMs>,
    watchers: HashMap<ConnectionId, mpsc::UnboundedSender<common::protocol::ServerMessage>>,
}

impl ModePreviewState {
    fn new() -> Self {
        Self {
            target_mode_id: None,
            empty_since: None,
            watchers: HashMap::new(),
        }
    }
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
    preview_state: Arc<RwLock<HashMap<ModeId, ModePreviewState>>>,
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
                mode_id.clone(),
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
            preview_state: Arc::new(RwLock::new(HashMap::new())),
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
    pub fn queue_target_players(&self, mode_id: &ModeId) -> Option<PlayerCount> {
        self.config_manager
            .modes
            .get(mode_id)
            .and_then(|mode| mode.queue_requirement())
    }

    /// Returns the current queue length for a mode.
    pub async fn queue_len(&self, mode_id: &ModeId) -> PlayerCount {
        self.queue_entries
            .read()
            .await
            .get(mode_id)
            .map(|queue| PlayerCount::new(queue.len() as u32))
            .unwrap_or_else(PlayerCount::zero)
    }

    /// Returns a snapshot of current queue entries and required size.
    pub async fn queue_snapshot(
        &self,
        mode_id: &ModeId,
    ) -> Option<(PlayerCount, Vec<MatchQueueEntry>)> {
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

    /// Sends an Init snapshot to a client with the public-mode identity.
    pub async fn send_init(
        &self,
        tx: &mpsc::UnboundedSender<common::protocol::ServerMessage>,
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
        state.mode_id = instance.public_mode_id().clone();
        let _ = tx.send(common::protocol::ServerMessage::Init {
            player_id,
            session_secret,
            state: Box::new(state),
            mode,
            pieces: instance.piece_config_snapshot(),
            shops: instance.shop_config_snapshot(),
        });
    }

    /// Registers a preview watcher for a matchmaking mode.
    pub async fn add_preview_connection(
        &self,
        mode_id: &ModeId,
        conn_id: ConnectionId,
        tx: mpsc::UnboundedSender<common::protocol::ServerMessage>,
    ) {
        let current_target = {
            let mut previews = self.preview_state.write().await;
            let preview = previews
                .entry(mode_id.clone())
                .or_insert_with(ModePreviewState::new);
            preview.watchers.insert(conn_id, tx.clone());
            preview.target_mode_id.clone()
        };

        let target = match current_target {
            Some(id) => self.get_game(&id).await,
            None => None,
        };
        let target = match target {
            Some(instance) => Some(instance),
            None => self.select_preview_target(mode_id).await,
        };

        let Some(instance) = target else {
            return;
        };

        instance
            .add_connection_channel(conn_id, tx.clone())
            .await;
        self.send_init(&tx, &instance, PlayerId::nil(), SessionSecret::nil())
            .await;

        let mut previews = self.preview_state.write().await;
        if let Some(preview) = previews.get_mut(mode_id) {
            if preview.target_mode_id.as_ref() != Some(instance.mode_id()) {
                preview.target_mode_id = Some(instance.mode_id().clone());
                preview.empty_since = None;
            }
        }
    }

    /// Removes a preview watcher from a matchmaking mode.
    pub async fn remove_preview_connection(&self, mode_id: &ModeId, conn_id: ConnectionId) {
        let target_id = {
            let mut previews = self.preview_state.write().await;
            let Some(preview) = previews.get_mut(mode_id) else {
                return;
            };
            preview.watchers.remove(&conn_id);
            let target_id = preview.target_mode_id.clone();
            if preview.watchers.is_empty() {
                previews.remove(mode_id);
            }
            target_id
        };

        if let Some(target_id) = target_id
            && let Some(instance) = self.get_game(&target_id).await
        {
            instance.remove_connection_channel(conn_id).await;
        }
    }

    /// Updates preview targets for matchmaking modes with active watchers.
    pub async fn tick_previews(&self) {
        let mode_ids = {
            let previews = self.preview_state.read().await;
            previews.keys().cloned().collect::<Vec<_>>()
        };

        for mode_id in mode_ids {
            self.refresh_preview_for_mode(&mode_id).await;
        }
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
        let required = self.queue_target_players(mode_id)?.as_u32() as usize;
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
        private_mode.queue_players = PlayerCount::zero();

        let match_instance = Arc::new(GameInstance::new(
            private_mode,
            mode_id.clone(),
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

    /// Re-evaluates preview selection for a matchmaking mode.
    pub async fn refresh_preview_for_mode(&self, mode_id: &ModeId) {
        let (watchers, current_target_id, empty_since) = {
            let previews = self.preview_state.read().await;
            let Some(preview) = previews.get(mode_id) else {
                return;
            };
            if preview.watchers.is_empty() {
                return;
            }
            (
                preview.watchers.clone(),
                preview.target_mode_id.clone(),
                preview.empty_since,
            )
        };

        let instances = self.preview_instances_for_mode(mode_id).await;
        let default_instance = self.get_game(mode_id).await;
        let latest_running = self.latest_running_instance(&instances).await;
        let now = now_ms();

        let current_instance = match &current_target_id {
            Some(id) => self.get_game(id).await,
            None => None,
        };

        let (desired_target, next_empty_since) = if let Some(latest) = latest_running {
            (Some(latest.mode_id().clone()), None)
        } else {
            let current_players = match &current_instance {
                Some(instance) => instance.player_count().await,
                None => PlayerCount::zero(),
            };
            let current_is_default = current_target_id.as_ref() == Some(mode_id);

            if current_players > PlayerCount::zero() {
                (current_target_id.clone(), None)
            } else if current_target_id.is_none() {
                (
                    default_instance.as_ref().map(|i| i.mode_id().clone()),
                    None,
                )
            } else if current_is_default {
                (current_target_id.clone(), None)
            } else {
                match empty_since {
                    None => (current_target_id.clone(), Some(now)),
                    Some(ts) if now - ts < DurationMs::from_millis(5000) => {
                        (current_target_id.clone(), Some(ts))
                    }
                    Some(_) => (
                        default_instance.as_ref().map(|i| i.mode_id().clone()),
                        None,
                    ),
                }
            }
        };

        let switch_required = desired_target != current_target_id;
        let empty_changed = next_empty_since != empty_since;

        if switch_required {
            if let Some(old_instance) = current_instance {
                for (conn_id, _) in &watchers {
                    old_instance.remove_connection_channel(*conn_id).await;
                }
            }

            if let Some(target_id) = &desired_target
                && let Some(new_instance) = self.get_game(target_id).await
            {
                for (conn_id, tx) in &watchers {
                    new_instance
                        .add_connection_channel(*conn_id, tx.clone())
                        .await;
                    self.send_init(tx, &new_instance, PlayerId::nil(), SessionSecret::nil())
                        .await;
                }
            }
        }

        if switch_required || empty_changed {
            let mut previews = self.preview_state.write().await;
            if let Some(preview) = previews.get_mut(mode_id) {
                preview.target_mode_id = desired_target;
                preview.empty_since = next_empty_since;
            }
        }

        if switch_required {
            self.cleanup_private_games().await;
        }
    }

    /// Selects the best preview target for a matchmaking mode.
    async fn select_preview_target(&self, mode_id: &ModeId) -> Option<Arc<GameInstance>> {
        let instances = self.preview_instances_for_mode(mode_id).await;
        if let Some(latest) = self.latest_running_instance(&instances).await {
            return Some(latest);
        }
        self.get_game(mode_id).await
    }

    /// Returns all instances that share the given public mode id.
    async fn preview_instances_for_mode(&self, mode_id: &ModeId) -> Vec<Arc<GameInstance>> {
        let games = self.games.read().await;
        games
            .values()
            .filter(|instance| instance.public_mode_id() == mode_id)
            .cloned()
            .collect()
    }

    /// Finds the most recently started running instance for the provided list.
    async fn latest_running_instance(
        &self,
        instances: &[Arc<GameInstance>],
    ) -> Option<Arc<GameInstance>> {
        let mut latest: Option<(TimestampMs, Arc<GameInstance>)> = None;
        for instance in instances {
            if instance.player_count().await <= PlayerCount::zero() {
                continue;
            }
            let started_at = instance.last_started_at().await;
            let should_take = match &latest {
                Some((prev_ts, _)) => started_at > *prev_ts,
                None => true,
            };
            if should_take {
                latest = Some((started_at, instance.clone()));
            }
        }
        latest.map(|(_, instance)| instance)
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

impl Default for ServerState {
    /// Provides a default server state by loading configuration.
    fn default() -> Self {
        Self::new()
    }
}
