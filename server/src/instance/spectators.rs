//! Named spectators of an instance and their arrival and departure.

use super::world::World;
use crate::types::{ClientTx, ConnectionId};
use common::protocol::ServerMessage;

/// Named spectators in the order they started watching.
#[derive(Default)]
pub(super) struct SpectatorRoster {
    entries: Vec<(ConnectionId, String)>,
}

impl SpectatorRoster {
    /// Adds or renames the spectator of `conn_id`.
    ///
    /// Returns true when the connection was not spectating before.
    fn insert(&mut self, conn_id: ConnectionId, name: String) -> bool {
        match self.entries.iter_mut().find(|(id, _)| *id == conn_id) {
            Some(entry) => {
                entry.1 = name;
                false
            }
            None => {
                self.entries.push((conn_id, name));
                true
            }
        }
    }

    /// Removes the spectator of `conn_id`, returning its name.
    fn remove(&mut self, conn_id: ConnectionId) -> Option<String> {
        let index = self.entries.iter().position(|(id, _)| *id == conn_id)?;
        Some(self.entries.remove(index).1)
    }

    /// Returns the name of the spectator of `conn_id`.
    pub(super) fn name_of(&self, conn_id: ConnectionId) -> Option<&str> {
        self.entries
            .iter()
            .find(|(id, _)| *id == conn_id)
            .map(|(_, name)| name.as_str())
    }

    /// Returns the number of spectators.
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns the spectator names in arrival order.
    pub(super) fn names(&self) -> Vec<String> {
        self.entries.iter().map(|(_, name)| name.clone()).collect()
    }

    /// Removes every spectator, returning their connections.
    fn drain(&mut self) -> Vec<ConnectionId> {
        self.entries.drain(..).map(|(id, _)| id).collect()
    }
}

impl World {
    /// Starts sending updates to a named spectator and announces it once.
    pub(super) fn add_spectator(&mut self, conn_id: ConnectionId, tx: ClientTx, name: String) {
        self.outbox.add_viewer(conn_id, tx);
        if self.spectators.insert(conn_id, name.clone()) {
            self.hooks.spectator_joins.push(name);
        }
    }

    /// Stops sending updates to a named spectator and announces its departure.
    pub(super) fn remove_spectator(&mut self, conn_id: ConnectionId) {
        self.outbox.remove_viewer(conn_id);
        if let Some(name) = self.spectators.remove(conn_id) {
            self.hooks.spectator_leaves.push(name);
        }
    }

    /// Returns true when a private match has no competing player left.
    ///
    /// Shared worlds never finish.
    pub(super) fn is_finished(&self) -> bool {
        self.setup.is_private && self.active_player_count() == 0
    }

    /// Tells the spectators of a finished match it ended and detaches them, so watching never
    /// keeps a finished match alive.
    pub(super) fn release_spectators_if_finished(&mut self) {
        if !self.is_finished() {
            return;
        }
        for conn_id in self.spectators.drain() {
            self.outbox
                .send_to_viewer(conn_id, ServerMessage::SpectateEnded);
            self.outbox.remove_viewer(conn_id);
        }
    }
}
