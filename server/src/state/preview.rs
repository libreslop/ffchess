//! Preview board watcher tracking and selection.

use super::ServerState;
use crate::instance::GameInstance;
use crate::time::now_ms;
use crate::types::ConnectionId;
use common::protocol::ServerMessage;
use common::types::{ModeId, PlayerCount, PlayerId, SessionSecret, TimestampMs};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Tracks preview watchers and the active preview target for a matchmaking mode.
pub(super) struct ModePreviewState {
    target_mode_id: Option<ModeId>,
    empty_since: Option<TimestampMs>,
    watchers: HashMap<ConnectionId, mpsc::UnboundedSender<ServerMessage>>,
    default_only: HashSet<ConnectionId>,
}

impl ModePreviewState {
    fn new() -> Self {
        Self {
            target_mode_id: None,
            empty_since: None,
            watchers: HashMap::new(),
            default_only: HashSet::new(),
        }
    }

    fn snapshot(&self) -> PreviewSnapshot {
        let dynamic_watchers = self
            .watchers
            .iter()
            .filter(|(conn_id, _)| !self.default_only.contains(*conn_id))
            .map(|(conn_id, tx)| (*conn_id, tx.clone()))
            .collect::<HashMap<_, _>>();
        PreviewSnapshot {
            dynamic_watchers,
            target_mode_id: self.target_mode_id.clone(),
            empty_since: self.empty_since,
        }
    }
}

/// Captures the state needed to evaluate a preview switch.
struct PreviewSnapshot {
    dynamic_watchers: HashMap<ConnectionId, mpsc::UnboundedSender<ServerMessage>>,
    target_mode_id: Option<ModeId>,
    empty_since: Option<TimestampMs>,
}

/// Represents the selected preview target and empty timer state.
struct PreviewSelection {
    target_mode_id: Option<ModeId>,
    empty_since: Option<TimestampMs>,
}

/// Captures the current target state while evaluating preview selection.
struct CurrentPreviewState<'a> {
    target_mode_id: Option<ModeId>,
    empty_since: Option<TimestampMs>,
    instance: Option<&'a Arc<GameInstance>>,
}

/// Bundles all inputs required to compute the next preview selection.
struct PreviewSelectionInput<'a> {
    mode_id: &'a ModeId,
    now: TimestampMs,
    current: CurrentPreviewState<'a>,
    default_instance: Option<&'a Arc<GameInstance>>,
    latest_running: Option<&'a Arc<GameInstance>>,
}

impl ServerState {
    /// Registers a preview watcher for a matchmaking mode.
    pub async fn add_preview_connection(
        &self,
        mode_id: &ModeId,
        conn_id: ConnectionId,
        tx: mpsc::UnboundedSender<ServerMessage>,
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

        instance.add_connection_channel(conn_id, tx.clone()).await;
        self.send_init(&tx, &instance, PlayerId::nil(), SessionSecret::nil())
            .await;

        let mut previews = self.preview_state.write().await;
        if let Some(preview) = previews.get_mut(mode_id)
            && preview.target_mode_id.as_ref() != Some(instance.mode_id())
        {
            preview.target_mode_id = Some(instance.mode_id().clone());
            preview.empty_since = None;
        }
    }

    /// Ensures a matchmaking connection is watching a preview board.
    pub async fn ensure_preview_connection(
        &self,
        mode_id: &ModeId,
        conn_id: ConnectionId,
        tx: mpsc::UnboundedSender<ServerMessage>,
    ) {
        let already_watching = {
            let previews = self.preview_state.read().await;
            previews
                .get(mode_id)
                .map(|preview| preview.watchers.contains_key(&conn_id))
                .unwrap_or(false)
        };

        if !already_watching {
            self.add_preview_connection(mode_id, conn_id, tx).await;
        }
    }

    /// Removes a preview watcher from a matchmaking mode.
    pub async fn remove_preview_connection(&self, mode_id: &ModeId, conn_id: ConnectionId) {
        let (target_id, was_default) = {
            let mut previews = self.preview_state.write().await;
            let Some(preview) = previews.get_mut(mode_id) else {
                return;
            };
            preview.watchers.remove(&conn_id);
            let was_default = preview.default_only.remove(&conn_id);
            let target_id = preview.target_mode_id.clone();
            if preview.watchers.is_empty() {
                previews.remove(mode_id);
            }
            (target_id, was_default)
        };

        if let Some(target_id) = target_id
            && let Some(instance) = self.get_game(&target_id).await
        {
            instance.remove_connection_channel(conn_id).await;
        }

        if was_default && let Some(default_instance) = self.get_game(mode_id).await {
            default_instance.remove_connection_channel(conn_id).await;
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

    /// Re-evaluates preview selection for a matchmaking mode.
    pub async fn refresh_preview_for_mode(&self, mode_id: &ModeId) {
        let snapshot = {
            let previews = self.preview_state.read().await;
            let Some(preview) = previews.get(mode_id) else {
                return;
            };
            if preview.watchers.is_empty() {
                return;
            }
            preview.snapshot()
        };

        if snapshot.dynamic_watchers.is_empty() {
            return;
        }

        let instances = self.preview_instances_for_mode(mode_id).await;
        let default_instance = self.get_game(mode_id).await;
        let latest_running = self.latest_running_instance(&instances).await;
        let now = now_ms();

        let current_instance = match &snapshot.target_mode_id {
            Some(id) => self.get_game(id).await,
            None => None,
        };

        let selection = self
            .select_preview_state(PreviewSelectionInput {
                mode_id,
                now,
                current: CurrentPreviewState {
                    target_mode_id: snapshot.target_mode_id.clone(),
                    empty_since: snapshot.empty_since,
                    instance: current_instance.as_ref(),
                },
                default_instance: default_instance.as_ref(),
                latest_running: latest_running.as_ref(),
            })
            .await;

        let switch_required = selection.target_mode_id != snapshot.target_mode_id;
        let empty_changed = selection.empty_since != snapshot.empty_since;

        if switch_required {
            if let Some(old_instance) = current_instance {
                for conn_id in snapshot.dynamic_watchers.keys() {
                    old_instance.remove_connection_channel(*conn_id).await;
                }
            }

            if let Some(target_id) = &selection.target_mode_id
                && let Some(new_instance) = self.get_game(target_id).await
            {
                for (conn_id, tx) in &snapshot.dynamic_watchers {
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
                preview.target_mode_id = selection.target_mode_id;
                preview.empty_since = selection.empty_since;
            }
        }

        if switch_required {
            self.cleanup_private_games().await;
        }
    }

    /// Forces a preview connection to either the default board or the active preview.
    pub async fn set_preview_default(
        &self,
        mode_id: &ModeId,
        conn_id: ConnectionId,
        tx: mpsc::UnboundedSender<ServerMessage>,
        enabled: bool,
    ) {
        if enabled && let Some(binding) = self.unbind_connection(conn_id).await {
            let (player_id, instance) = binding.into_parts();
            instance.detach_player(player_id).await;
            self.cleanup_private_games().await;
        }
        self.ensure_preview_connection(mode_id, conn_id, tx.clone())
            .await;

        let (current_target_id, was_default) = {
            let previews = self.preview_state.read().await;
            let Some(preview) = previews.get(mode_id) else {
                return;
            };
            (
                preview.target_mode_id.clone(),
                preview.default_only.contains(&conn_id),
            )
        };

        if enabled == was_default {
            return;
        }

        {
            let mut previews = self.preview_state.write().await;
            if let Some(preview) = previews.get_mut(mode_id) {
                if enabled {
                    preview.default_only.insert(conn_id);
                } else {
                    preview.default_only.remove(&conn_id);
                }
            }
        }

        if enabled {
            if let Some(target_id) = current_target_id
                && let Some(instance) = self.get_game(&target_id).await
            {
                instance.remove_connection_channel(conn_id).await;
            }

            if let Some(default_instance) = self.get_game(mode_id).await {
                default_instance
                    .add_connection_channel(conn_id, tx.clone())
                    .await;
                self.send_init(
                    &tx,
                    &default_instance,
                    PlayerId::nil(),
                    SessionSecret::nil(),
                )
                .await;
            }
            return;
        }

        if let Some(default_instance) = self.get_game(mode_id).await {
            default_instance.remove_connection_channel(conn_id).await;
        }

        let target = match current_target_id {
            Some(id) => self.get_game(&id).await,
            None => self.select_preview_target(mode_id).await,
        };

        if let Some(instance) = target {
            instance.add_connection_channel(conn_id, tx.clone()).await;
            self.send_init(&tx, &instance, PlayerId::nil(), SessionSecret::nil())
                .await;

            let mut previews = self.preview_state.write().await;
            if let Some(preview) = previews.get_mut(mode_id)
                && preview.target_mode_id.as_ref() != Some(instance.mode_id())
            {
                preview.target_mode_id = Some(instance.mode_id().clone());
                preview.empty_since = None;
            }
        }
    }

    async fn select_preview_state(&self, input: PreviewSelectionInput<'_>) -> PreviewSelection {
        let PreviewSelectionInput {
            mode_id,
            now,
            current,
            default_instance,
            latest_running,
        } = input;
        let switch_delay = self
            .config_manager
            .modes
            .get(mode_id)
            .map(|mode| mode.preview_switch_delay_ms)
            .unwrap_or_else(common::models::GameModeConfig::default_preview_switch_delay_ms);
        let current_players = match current.instance {
            Some(instance) => instance.player_count().await,
            None => PlayerCount::zero(),
        };
        let current_is_default = current.target_mode_id.as_ref() == Some(mode_id);

        if current.target_mode_id.is_some() && !current_is_default {
            if current_players > PlayerCount::zero() {
                return PreviewSelection {
                    target_mode_id: current.target_mode_id,
                    empty_since: None,
                };
            }

            match current.empty_since {
                None => {
                    return PreviewSelection {
                        target_mode_id: current.target_mode_id,
                        empty_since: Some(now),
                    };
                }
                Some(ts) if now - ts < switch_delay => {
                    return PreviewSelection {
                        target_mode_id: current.target_mode_id,
                        empty_since: Some(ts),
                    };
                }
                Some(_) => {}
            }

            return PreviewSelection {
                target_mode_id: latest_running
                    .map(|instance| instance.mode_id().clone())
                    .or_else(|| default_instance.map(|instance| instance.mode_id().clone())),
                empty_since: None,
            };
        }

        if let Some(latest) = latest_running {
            return PreviewSelection {
                target_mode_id: Some(latest.mode_id().clone()),
                empty_since: None,
            };
        }

        if current.target_mode_id.is_none() {
            return PreviewSelection {
                target_mode_id: default_instance.map(|instance| instance.mode_id().clone()),
                empty_since: None,
            };
        }

        PreviewSelection {
            target_mode_id: current.target_mode_id,
            empty_since: None,
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
}
