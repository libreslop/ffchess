//! Named spectators: connections watching a board without playing on it.
//!
//! Unlike the anonymous watchers behind the menu, named spectators are announced in chat and
//! listed to everyone on the board. Spectating a private match ends with the match: the
//! instance releases its spectators itself, and the lobby forgets them on the next tick.

use super::bindings::Binding;
use super::connection::Connection;
use super::lobby::Lobby;
use super::requests::MAX_GUEST_NAME_CHARS;
use crate::instance::{GameInstance, Spectator};
use crate::text::UserText;
use common::models::MatchSummary;
use common::protocol::{GameError, ServerMessage, SpectateTarget};
use common::types::{PlayerId, SessionSecret};
use std::cmp::Reverse;
use std::sync::Arc;

impl Lobby {
    /// Makes a connection a named spectator of `target`.
    ///
    /// Players still competing cannot spectate. A client whose `player_id` plays on the target
    /// board from another connection is refused: it goes back to the menu board and gets an
    /// error. A target that does not exist or already finished answers
    /// [`ServerMessage::SpectateEnded`]; the connection then watches nothing until it leaves.
    pub(super) fn spectate(
        &mut self,
        conn: &Connection,
        name: String,
        target: SpectateTarget,
        player_id: Option<PlayerId>,
    ) {
        let own_binding = conn.binding.get();
        if own_binding.as_ref().is_some_and(Binding::is_competing) {
            return;
        }
        let name = self.spectator_name(name);
        // Resolved before the cleanup below drops the match the connection played in. The
        // client has left its session either way, so the cleanup always happens.
        let instance = self.spectate_target(conn, &target);
        // This connection's own finished session is dropped below, so only other tabs count.
        let own_id = own_binding.map(|binding| binding.player_id);
        let plays_elsewhere = player_id.is_some_and(|player_id| {
            Some(player_id) != own_id
                && instance
                    .as_ref()
                    .is_some_and(|instance| instance.has_active_session(player_id))
        });
        if plays_elsewhere {
            self.leave(conn);
            return conn.send_error(GameError::Custom {
                title: "Already Playing".to_string(),
                message: "You are playing in this game in another tab".to_string(),
            });
        }

        self.stop_spectating(conn);
        conn.binding.detach();
        self.dequeue(&conn.mode_id, conn.id);
        if conn.is_queue_mode {
            self.remove_preview_watcher(&conn.mode_id, conn.id);
        }
        self.cleanup_private_games();

        let Some(instance) = instance else {
            return conn.send(ServerMessage::SpectateEnded);
        };

        instance.add_spectator(conn.id, conn.tx.clone(), Spectator { name, player_id });
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

    /// Cleans a requested spectator name, shortening long names and generating empty ones.
    ///
    /// Spectating has no error screen to show a rejected name on, so names are fixed up
    /// instead of rejected.
    fn spectator_name(&self, input: String) -> String {
        input
            .short_name(MAX_GUEST_NAME_CHARS)
            .unwrap_or_else(|| self.generate_name())
    }

    /// Resolves the board a spectate request points at, if it can still be watched.
    fn spectate_target(
        &self,
        conn: &Connection,
        target: &SpectateTarget,
    ) -> Option<Arc<GameInstance>> {
        let instance = match target {
            SpectateTarget::Own => match conn.binding.get() {
                Some(binding) => Some(binding.instance),
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
