//! Registry of instances and the connections attached to them.

use super::bindings::Binding;
use super::connection::Connection;
use super::preview::PreviewWatchers;
use super::queue::QueueEntry;
use crate::config::ConfigManager;
use crate::instance::GameInstance;
use crate::types::{ClientTx, ConnectionId};
use common::models::GameModeConfig;
use common::types::{ModeId, PlayerId, SessionSecret};
use std::collections::HashMap;
use std::sync::Arc;

/// All instances, queues, previews, and connection bindings of the server.
pub(super) struct Lobby {
    pub(super) config: Arc<ConfigManager>,
    /// Public instances keyed by mode id, private matches keyed by their own id.
    pub(super) games: HashMap<ModeId, Arc<GameInstance>>,
    pub(super) queues: HashMap<ModeId, Vec<QueueEntry>>,
    pub(super) bindings: HashMap<ConnectionId, Binding>,
    pub(super) previews: HashMap<ModeId, PreviewWatchers>,
}

impl Lobby {
    /// Creates the lobby with one public instance per configured mode.
    pub(super) fn new(config: Arc<ConfigManager>) -> Self {
        let games = config
            .modes
            .iter()
            .map(|(mode_id, mode)| {
                let instance = GameInstance::public(
                    mode.clone(),
                    config.catalog.clone(),
                    config.global.chat_ttl(),
                );
                instance.spawn_initial_shops();
                (mode_id.clone(), Arc::new(instance))
            })
            .collect();
        Self {
            config,
            games,
            queues: HashMap::new(),
            bindings: HashMap::new(),
            previews: HashMap::new(),
        }
    }

    /// Returns every live instance.
    pub(super) fn instances(&self) -> Vec<Arc<GameInstance>> {
        self.games.values().cloned().collect()
    }

    /// Returns the public instance of a mode.
    pub(super) fn public_instance(&self, mode_id: &ModeId) -> Option<&Arc<GameInstance>> {
        self.games
            .get(mode_id)
            .filter(|instance| !instance.is_private())
    }

    /// Returns the config of a public mode.
    pub(super) fn mode(&self, mode_id: &ModeId) -> Option<&GameModeConfig> {
        self.config.modes.get(mode_id)
    }

    /// Returns true when the mode matches players through a queue.
    pub(super) fn is_queue_mode(&self, mode_id: &ModeId) -> bool {
        self.mode(mode_id)
            .is_some_and(|mode| mode.queue_requirement().is_some())
    }

    /// Returns all instances belonging to a public mode, including its private matches.
    pub(super) fn instances_of(
        &self,
        mode_id: &ModeId,
    ) -> impl Iterator<Item = &Arc<GameInstance>> {
        self.games
            .values()
            .filter(move |instance| instance.public_mode_id() == mode_id)
    }

    /// Sends the Init snapshot of `instance` to `tx`.
    pub(super) fn send_init(
        &self,
        tx: &ClientTx,
        instance: &GameInstance,
        player_id: PlayerId,
        secret: SessionSecret,
    ) {
        let message = instance.init_message(player_id, secret, self.config.global.sync_interval_ms);
        let _ = tx.try_send(message);
    }

    /// Makes `conn_id` a spectator of `instance` and sends it the board.
    pub(super) fn watch(&self, instance: &GameInstance, conn_id: ConnectionId, tx: &ClientTx) {
        instance.add_viewer(conn_id, tx.clone());
        self.send_init(tx, instance, PlayerId::nil(), SessionSecret::nil());
    }

    /// Makes a public-mode connection a spectator of its mode's shared world.
    pub(super) fn watch_public(&self, conn: &Connection) {
        if let Some(instance) = self.public_instance(&conn.mode_id) {
            self.watch(instance, conn.id, &conn.tx);
        }
    }

    /// Links a connection to the player it controls.
    pub(super) fn bind(
        &mut self,
        conn_id: ConnectionId,
        player_id: PlayerId,
        instance: Arc<GameInstance>,
    ) {
        self.bindings.insert(
            conn_id,
            Binding {
                player_id,
                instance,
            },
        );
    }

    /// Removes the player of a connection from its match so it can join again elsewhere.
    pub(super) fn detach_binding(&mut self, conn_id: ConnectionId) {
        if let Some(binding) = self.bindings.remove(&conn_id) {
            binding.instance.detach(binding.player_id);
        }
    }

    /// Drops private matches that nobody plays in or watches anymore.
    pub(super) fn cleanup_private_games(&mut self) {
        self.games
            .retain(|_, instance| !instance.is_private() || !instance.is_abandoned());
    }
}
