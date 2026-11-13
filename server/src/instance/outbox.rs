//! Outbound channels of everyone watching or playing in an instance.

use crate::types::{ClientTx, ConnectionId};
use common::protocol::ServerMessage;
use common::types::PlayerId;
use std::collections::HashMap;
use tokio::sync::mpsc::error::TrySendError;

/// Message channels of spectators and players of one instance.
#[derive(Default)]
pub(super) struct Outbox {
    viewers: HashMap<ConnectionId, ClientTx>,
    players: HashMap<PlayerId, ClientTx>,
}

impl Outbox {
    /// Registers a spectator connection.
    pub(super) fn add_viewer(&mut self, conn_id: ConnectionId, tx: ClientTx) {
        self.viewers.insert(conn_id, tx);
    }

    /// Unregisters a spectator connection.
    pub(super) fn remove_viewer(&mut self, conn_id: ConnectionId) {
        self.viewers.remove(&conn_id);
    }

    /// Returns true when `conn_id` is registered as a spectator.
    pub(super) fn has_viewer(&self, conn_id: ConnectionId) -> bool {
        self.viewers.contains_key(&conn_id)
    }

    /// Registers the channel of a player.
    pub(super) fn add_player(&mut self, player_id: PlayerId, tx: ClientTx) {
        self.players.insert(player_id, tx);
    }

    /// Unregisters the channel of a player.
    pub(super) fn remove_player(&mut self, player_id: PlayerId) {
        self.players.remove(&player_id);
    }

    /// Returns true when `player_id` has a registered channel.
    pub(super) fn has_player(&self, player_id: PlayerId) -> bool {
        self.players.contains_key(&player_id)
    }

    /// Returns the ids of players with a registered channel.
    pub(super) fn player_ids(&self) -> Vec<PlayerId> {
        self.players.keys().copied().collect()
    }

    /// Returns true when nobody is attached to the instance.
    pub(super) fn is_empty(&self) -> bool {
        self.viewers.is_empty() && self.players.is_empty()
    }

    /// Sends `msg` to one player, if connected.
    pub(super) fn send_to_player(&self, player_id: PlayerId, msg: ServerMessage) {
        if let Some(tx) = self.players.get(&player_id) {
            let _ = tx.try_send(msg);
        }
    }

    /// Sends `msg` to every player and spectator.
    ///
    /// Channels whose connection is gone are dropped. A full channel only misses this message;
    /// state updates are full snapshots, so a lagging client catches up on the next one.
    pub(super) fn broadcast(&mut self, msg: &ServerMessage) {
        let still_open =
            |tx: &mut ClientTx| !matches!(tx.try_send(msg.clone()), Err(TrySendError::Closed(_)));
        self.players.retain(|_, tx| still_open(tx));
        self.viewers.retain(|_, tx| still_open(tx));
    }
}
