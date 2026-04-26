//! Matchmaking queue tracking for queue-based modes.

use super::ServerState;
use crate::instance::GameInstance;
use crate::types::ConnectionId;
use common::protocol::ServerMessage;
use common::types::{KitId, ModeId, PlayerCount};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Queue entry for matchmaking modes.
#[derive(Clone)]
pub struct MatchQueueEntry {
    conn_id: ConnectionId,
    tx: mpsc::Sender<ServerMessage>,
    name: String,
    kit_name: KitId,
}

impl MatchQueueEntry {
    /// Creates a new matchmaking queue entry.
    pub fn new(
        conn_id: ConnectionId,
        tx: mpsc::Sender<ServerMessage>,
        name: String,
        kit_name: KitId,
    ) -> Self {
        Self {
            conn_id,
            tx,
            name,
            kit_name,
        }
    }

    /// Returns the queued connection id.
    pub fn conn_id(&self) -> ConnectionId {
        self.conn_id
    }

    /// Returns the channel sender for this queued client.
    pub fn tx(&self) -> &mpsc::Sender<ServerMessage> {
        &self.tx
    }

    /// Returns the queued player name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the queued kit name.
    pub fn kit_name(&self) -> &KitId {
        &self.kit_name
    }

    /// Consumes the entry into its owned components.
    pub fn into_parts(self) -> (ConnectionId, mpsc::Sender<ServerMessage>, String, KitId) {
        (self.conn_id, self.tx, self.name, self.kit_name)
    }
}

/// Result of attempting to enqueue a player into a matchmaking queue.
pub enum QueueJoinResult {
    Waiting,
    Matched {
        match_instance: Arc<GameInstance>,
        players: Vec<MatchQueueEntry>,
    },
}

impl ServerState {
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

    /// Removes a connection from a mode queue if present.
    pub async fn remove_from_queue(&self, mode_id: &ModeId, conn_id: ConnectionId) -> bool {
        let mut queues = self.queue_entries.write().await;
        let (removed, became_empty) = {
            let Some(entries) = queues.get_mut(mode_id) else {
                return false;
            };
            let before = entries.len();
            entries.retain(|entry| entry.conn_id != conn_id);
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
            if let Some(idx) = queue
                .iter()
                .position(|queued| queued.conn_id == entry.conn_id)
            {
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
            self.config_manager
                .modes
                .get(mode_id)
                .and_then(|mode| mode.queue_layout.clone())
                .map(Arc::new),
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
}
