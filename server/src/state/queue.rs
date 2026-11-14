//! Matchmaking queues of queue-based modes.

use super::connection::Connection;
use super::lobby::Lobby;
use crate::instance::{GameInstance, JoinRequest};
use crate::types::ConnectionId;
use common::protocol::{GameError, ServerMessage};
use common::types::{KitId, ModeId, PlayerCount, QueuePosition};
use std::sync::Arc;

/// A connection waiting for a match.
pub(super) struct QueueEntry {
    conn: Connection,
    name: String,
    kit_name: KitId,
}

impl Lobby {
    /// Returns how many connections wait in the queue of a mode.
    pub(super) fn queue_len(&self, mode_id: &ModeId) -> PlayerCount {
        PlayerCount::new(self.queues.get(mode_id).map_or(0, Vec::len) as u32)
    }

    /// Returns the name a queued connection entered.
    pub(super) fn queued_name(&self, mode_id: &ModeId, conn_id: ConnectionId) -> Option<String> {
        self.queues
            .get(mode_id)?
            .iter()
            .find(|entry| entry.conn.id == conn_id)
            .map(|entry| entry.name.clone())
    }

    /// Puts a connection in its mode's queue and starts a match once enough players wait.
    pub(super) fn enqueue(&mut self, conn: &Connection, name: String, kit_name: KitId) {
        let Some(required) = self
            .mode(&conn.mode_id)
            .and_then(|mode| mode.queue_requirement())
        else {
            conn.send_error(GameError::Internal(
                "Matchmaking mode is not configured".to_string(),
            ));
            return;
        };
        self.ensure_preview_watcher(conn);

        let queue = self.queues.entry(conn.mode_id.clone()).or_default();
        queue.retain(|entry| entry.conn.id != conn.id);
        queue.push(QueueEntry {
            conn: conn.clone(),
            name,
            kit_name,
        });
        let required = required.as_u32() as usize;
        if queue.len() >= required {
            let players: Vec<QueueEntry> = queue.drain(..required).collect();
            self.start_match(&conn.mode_id, players);
            self.refresh_preview(&conn.mode_id);
        }
        self.broadcast_queue_state(&conn.mode_id);
    }

    /// Removes a connection from its mode's queue, updating everyone still waiting.
    pub(super) fn dequeue(&mut self, mode_id: &ModeId, conn_id: ConnectionId) {
        let Some(queue) = self.queues.get_mut(mode_id) else {
            return;
        };
        let before = queue.len();
        queue.retain(|entry| entry.conn.id != conn_id);
        let removed = queue.len() != before;
        if queue.is_empty() {
            self.queues.remove(mode_id);
        }
        if removed {
            self.broadcast_queue_state(mode_id);
        }
    }

    /// Creates a private match for `players` and moves them into it.
    fn start_match(&mut self, mode_id: &ModeId, players: Vec<QueueEntry>) {
        let Some(template) = self.mode(mode_id) else {
            return;
        };
        let instance = Arc::new(GameInstance::private_match(
            template,
            self.config.catalog.clone(),
            self.config.global.chat_ttl(),
        ));
        instance.start_countdown();
        instance.spawn_initial_shops();
        self.games
            .insert(instance.mode_id().clone(), instance.clone());

        for QueueEntry {
            conn,
            name,
            kit_name,
        } in players
        {
            self.remove_preview_watcher(mode_id, conn.id);
            let joined = instance.join(JoinRequest {
                name,
                kit_name,
                tx: conn.tx.clone(),
                player_id: None,
                session_secret: None,
            });
            match joined {
                Ok((player_id, secret)) => {
                    conn.binding.bind(player_id, instance.clone());
                    self.send_init(&conn.tx, &instance, player_id, secret);
                }
                Err(error) => conn.send_error(error),
            }
        }
    }

    /// Tells every queued connection of a mode its position and the queue size.
    fn broadcast_queue_state(&self, mode_id: &ModeId) {
        let Some(required_players) = self.mode(mode_id).and_then(|mode| mode.queue_requirement())
        else {
            return;
        };
        let Some(queue) = self.queues.get(mode_id) else {
            return;
        };
        let queued_players = PlayerCount::new(queue.len() as u32);
        for (index, entry) in queue.iter().enumerate() {
            entry.conn.send(ServerMessage::QueueState {
                position_in_queue: QueuePosition::new(index as u32 + 1),
                queued_players,
                required_players,
            });
        }
    }
}
