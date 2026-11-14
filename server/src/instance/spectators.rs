//! Named spectators of an instance and their arrival and departure.

use super::world::World;
use crate::outbound::ClientTx;
use crate::types::ConnectionId;
use common::protocol::ServerMessage;
use common::types::PlayerId;

/// A connection watching the board as a named spectator.
pub struct Spectator {
    pub name: String,
    /// Stored player id of the spectating client, used to keep it off boards it plays on.
    pub player_id: Option<PlayerId>,
}

/// Named spectators in the order they started watching.
#[derive(Default)]
pub(super) struct SpectatorRoster {
    entries: Vec<(ConnectionId, Spectator)>,
}

impl SpectatorRoster {
    /// Adds or replaces the spectator of `conn_id`.
    ///
    /// Returns true when the connection was not spectating before.
    fn insert(&mut self, conn_id: ConnectionId, spectator: Spectator) -> bool {
        match self.entries.iter_mut().find(|(id, _)| *id == conn_id) {
            Some(entry) => {
                entry.1 = spectator;
                false
            }
            None => {
                self.entries.push((conn_id, spectator));
                true
            }
        }
    }

    /// Removes the spectator of `conn_id`, returning its name.
    fn remove(&mut self, conn_id: ConnectionId) -> Option<String> {
        let index = self.entries.iter().position(|(id, _)| *id == conn_id)?;
        Some(self.entries.remove(index).1.name)
    }

    /// Returns the name of the spectator of `conn_id`.
    pub(super) fn name_of(&self, conn_id: ConnectionId) -> Option<&str> {
        self.entries
            .iter()
            .find(|(id, _)| *id == conn_id)
            .map(|(_, spectator)| spectator.name.as_str())
    }

    /// Returns the number of spectators.
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns the spectator names in arrival order.
    pub(super) fn names(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|(_, spectator)| spectator.name.clone())
            .collect()
    }

    /// Returns the connections spectating under `player_id`.
    fn connections_of(&self, player_id: PlayerId) -> Vec<ConnectionId> {
        self.entries
            .iter()
            .filter(|(_, spectator)| spectator.player_id == Some(player_id))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Removes every spectator, returning their connections.
    fn drain(&mut self) -> Vec<ConnectionId> {
        self.entries.drain(..).map(|(id, _)| id).collect()
    }
}

impl World {
    /// Starts sending updates to a named spectator and announces it once.
    pub(super) fn add_spectator(
        &mut self,
        conn_id: ConnectionId,
        tx: ClientTx,
        spectator: Spectator,
    ) {
        self.outbox.add_viewer(conn_id, tx);
        let name = spectator.name.clone();
        if self.spectators.insert(conn_id, spectator) {
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

    /// Ends spectating for the other tabs of a player who just joined this board, so nobody
    /// watches a match they play in.
    pub(super) fn release_spectators_of(&mut self, player_id: PlayerId) {
        for conn_id in self.spectators.connections_of(player_id) {
            self.outbox
                .send_to_viewer(conn_id, ServerMessage::SpectateEnded);
            self.remove_spectator(conn_id);
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
