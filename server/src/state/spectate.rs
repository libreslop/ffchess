//! Named spectators: connections watching a board without playing on it.
//!
//! Unlike the anonymous watchers behind the menu, named spectators are announced in chat and
//! listed to everyone on the board. Spectating a private match ends with the match: the
//! instance releases its spectators itself, and the lobby forgets them on the next tick.

use super::connection::Connection;
use super::lobby::Lobby;
use super::requests::MAX_GUEST_NAME_CHARS;
use crate::instance::GameInstance;
use common::models::MatchSummary;
use common::protocol::{ServerMessage, SpectateTarget};
use common::types::{PlayerId, SessionSecret};
use std::cmp::Reverse;
use std::sync::Arc;

impl Lobby {
    /// Makes a connection a named spectator of `target`.
    ///
    /// Players still competing cannot spectate. A target that does not exist or already
    /// finished answers [`ServerMessage::SpectateEnded`]; the connection then watches nothing
    /// until it leaves.
    pub(super) fn spectate(&mut self, conn: &Connection, name: String, target: SpectateTarget) {
        if let Some(binding) = self.bindings.get(&conn.id)
            && binding.instance.is_competing(binding.player_id)
        {
            return;
        }
        let name = self.spectator_name(name);
        // Resolved before the cleanup below drops the match the connection played in. The
        // client has left its session either way, so the cleanup always happens.
        let instance = self.spectate_target(conn, &target);

        self.stop_spectating(conn);
        self.detach_binding(conn.id);
        self.dequeue(&conn.mode_id, conn.id);
        if conn.is_queue_mode {
            self.remove_preview_watcher(&conn.mode_id, conn.id);
        }
        self.cleanup_private_games();

        let Some(instance) = instance else {
            return conn.send(ServerMessage::SpectateEnded);
        };

        instance.add_spectator(conn.id, conn.tx.clone(), name);
        self.send_init(&conn.tx, &instance, PlayerId::nil(), SessionSecret::nil());
        self.spectators.insert(conn.id, instance);
    }

    /// Stops spectating, announcing the departure on the spectated board.
    pub(super) fn stop_spectating(&mut self, conn: &Connection) {
        if let Some(instance) = self.spectators.remove(&conn.id) {
            instance.remove_spectator(conn.id);
        }
    }

    /// Returns true while a connection spectates a board.
    pub(super) fn is_spectating(&self, conn: &Connection) -> bool {
        self.spectators.contains_key(&conn.id)
    }

    /// Returns the board a connection spectates.
    pub(super) fn spectated_instance(&self, conn: &Connection) -> Option<&Arc<GameInstance>> {
        self.spectators.get(&conn.id)
    }

    /// Forgets spectators whose match released them when it finished.
    pub(super) fn prune_spectators(&mut self) {
        self.spectators
            .retain(|conn_id, instance| instance.spectator_name(*conn_id).is_some());
    }

    /// Sends a connection the running matches of its queue mode, newest first.
    pub(super) fn send_match_list(&self, conn: &Connection) {
        let mut running: Vec<&Arc<GameInstance>> = self
            .instances_of(&conn.mode_id)
            .filter(|instance| instance.is_private() && !instance.is_finished())
            .collect();
        running.sort_by_key(|instance| Reverse(instance.last_started_at()));
        let matches: Vec<MatchSummary> = running
            .into_iter()
            .map(|instance| instance.match_summary())
            .filter(|summary| !summary.players.is_empty())
            .collect();
        conn.send(ServerMessage::MatchList { matches });
    }

    /// Trims a requested spectator name, shortening long names and generating empty ones.
    ///
    /// Spectating has no error screen to show a rejected name on, so names are fixed up
    /// instead of rejected.
    fn spectator_name(&self, input: String) -> String {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            self.generate_name()
        } else {
            trimmed.chars().take(MAX_GUEST_NAME_CHARS).collect()
        }
    }

    /// Resolves the board a spectate request points at, if it can still be watched.
    fn spectate_target(
        &self,
        conn: &Connection,
        target: &SpectateTarget,
    ) -> Option<Arc<GameInstance>> {
        let instance = match target {
            SpectateTarget::Own => match self.bindings.get(&conn.id) {
                Some(binding) => Some(binding.instance.clone()),
                None if conn.is_queue_mode => None,
                None => self.public_instance(&conn.mode_id).cloned(),
            },
            SpectateTarget::Match(match_id) => self
                .games
                .get(match_id)
                .filter(|instance| {
                    instance.is_private() && instance.public_mode_id() == &conn.mode_id
                })
                .cloned(),
        };
        instance.filter(|instance| !instance.is_finished())
    }
}
