//! Server-wide registry of instances, queues, previews, and connections.
//!
//! Everything lives in one [`Lobby`] behind one mutex. Lobby operations may call into game
//! instances (which take their own lock), but instances never call back into the lobby, so the
//! lock order is always lobby before instance. Moves skip the lobby: they only need the player
//! a connection controls, which its binding slot holds.

mod bindings;
mod connection;
mod listing;
mod lobby;
mod preview;
mod queue;
mod requests;
mod spectate;

pub use connection::Connection;

use crate::config::ConfigManager;
use crate::instance::GameInstance;
use crate::outbound::ClientTx;
use crate::time::now_ms;
use common::models::ModeSummary;
use common::protocol::{ClientMessage, ServerMessage};
use common::types::ModeId;
use lobby::Lobby;
use requests::JoinForm;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Shared server state: configuration plus the lobby of live instances.
pub struct ServerState {
    config: Arc<ConfigManager>,
    lobby: Mutex<Lobby>,
}

impl ServerState {
    /// Loads configuration and creates one public instance per configured mode.
    pub fn new() -> Self {
        let config = Arc::new(ConfigManager::load(&crate::paths::config_dir()));
        Self {
            lobby: Mutex::new(Lobby::new(config.clone())),
            config,
        }
    }

    /// Locks the lobby, ignoring poisoning like the instance locks do.
    fn lobby(&self) -> MutexGuard<'_, Lobby> {
        self.lobby.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Returns the loaded configuration.
    pub fn config(&self) -> &ConfigManager {
        &self.config
    }

    /// Returns the public instance of a mode.
    pub fn public_instance(&self, mode_id: &ModeId) -> Option<Arc<GameInstance>> {
        self.lobby().public_instance(mode_id).cloned()
    }

    /// Returns the current status of every public mode.
    pub fn mode_summaries(&self) -> Vec<ModeSummary> {
        self.lobby().mode_summaries()
    }

    /// Registers a new websocket connection to `mode_id` and sends it the board to watch.
    ///
    /// Returns `None` when the mode does not exist.
    pub fn open_connection(&self, mode_id: ModeId, tx: ClientTx) -> Option<Connection> {
        self.lobby().open_connection(mode_id, tx)
    }

    /// Handles one message received from a connection.
    ///
    /// Moves and pings only concern the connection itself and skip the lobby lock, so a flood
    /// of other requests does not slow down play.
    pub fn handle_message(&self, conn: &Connection, message: ClientMessage) {
        match message {
            ClientMessage::QueueAction(request) => conn.submit_action(request),
            ClientMessage::ClearActions { piece_id } => conn.clear_actions(piece_id),
            ClientMessage::Ping(client_time) => {
                conn.send(ServerMessage::Pong(client_time, now_ms()));
            }
            ClientMessage::Join {
                name,
                kit_name,
                player_id,
                session_secret,
            } => self.lobby().join(
                conn,
                JoinForm {
                    name,
                    kit_name,
                    player_id,
                    session_secret,
                },
            ),
            ClientMessage::Leave => self.lobby().leave(conn),
            ClientMessage::ListMatches => self.lobby().send_match_list(conn),
            ClientMessage::Spectate {
                name,
                target,
                player_id,
            } => self.lobby().spectate(conn, name, target, player_id),
            ClientMessage::SetPreviewDefault { enabled } => {
                self.lobby().set_preview_default(conn, enabled);
            }
            ClientMessage::Chat { name_hint, message } => {
                self.lobby().chat(conn, name_hint, message);
            }
        }
    }

    /// Releases everything a closed connection held.
    pub fn close_connection(&self, conn: &Connection) {
        self.lobby().close_connection(conn);
    }

    /// Advances every instance by one tick, then updates previews and drops finished matches.
    pub fn tick(&self) {
        let instances = self.lobby().instances();
        for instance in instances {
            instance.tick();
        }
        let mut lobby = self.lobby();
        lobby.refresh_previews();
        lobby.prune_spectators();
        lobby.cleanup_private_games();
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}
