//! Handling of client requests on behalf of one connection.

use super::connection::Connection;
use super::lobby::Lobby;
use crate::instance::{GameInstance, JoinRequest};
use crate::time::now_ms;
use crate::types::{ClientTx, ConnectionId};
use common::protocol::{ChatLine, ClientMessage, GameError, ServerMessage};
use common::types::{ColorHex, KitId, ModeId, PlayerId, SessionSecret};
use rand::seq::SliceRandom;
use std::sync::Arc;

/// Longest accepted player name, in bytes.
const MAX_NAME_LEN: usize = 32;

/// Longest chat sender name used for guests, in characters.
pub(super) const MAX_GUEST_NAME_CHARS: usize = 32;

/// Chat color of players who are not in a match.
const GUEST_CHAT_COLOR: &str = "#555555";

/// A join request as received from a client.
struct JoinForm {
    name: String,
    kit_name: KitId,
    player_id: Option<PlayerId>,
    session_secret: Option<SessionSecret>,
}

impl Lobby {
    /// Registers a new connection and shows it its mode's board.
    pub(super) fn open_connection(&mut self, mode_id: ModeId, tx: ClientTx) -> Option<Connection> {
        self.public_instance(&mode_id)?;
        let conn = Connection {
            id: ConnectionId::new(),
            is_queue_mode: self.is_queue_mode(&mode_id),
            mode_id,
            tx,
        };
        if conn.is_queue_mode {
            self.ensure_preview_watcher(&conn);
        } else {
            self.watch_public(&conn);
        }
        Some(conn)
    }

    /// Dispatches one client message.
    pub(super) fn handle_message(&mut self, conn: &Connection, message: ClientMessage) {
        match message {
            ClientMessage::Join {
                name,
                kit_name,
                player_id,
                session_secret,
            } => self.join(
                conn,
                JoinForm {
                    name,
                    kit_name,
                    player_id,
                    session_secret,
                },
            ),
            ClientMessage::Leave => self.leave(conn),
            ClientMessage::ListMatches => self.send_match_list(conn),
            ClientMessage::Spectate { name, target } => self.spectate(conn, name, target),
            ClientMessage::QueueAction(request) => {
                if let Some(binding) = self.bindings.get(&conn.id) {
                    binding.instance.submit_action(binding.player_id, request);
                }
            }
            ClientMessage::ClearActions { piece_id } => {
                if let Some(binding) = self.bindings.get(&conn.id) {
                    binding.instance.clear_actions(binding.player_id, piece_id);
                }
            }
            ClientMessage::SetPreviewDefault { enabled } => {
                if conn.is_queue_mode && !self.is_spectating(conn) {
                    self.set_preview_default(conn, enabled);
                }
            }
            ClientMessage::Chat { name_hint, message } => self.chat(conn, name_hint, message),
            ClientMessage::Ping(client_time) => {
                conn.send(ServerMessage::Pong(client_time, now_ms()));
            }
        }
    }

    /// Releases everything a closed connection held.
    pub(super) fn close_connection(&mut self, conn: &Connection) {
        self.stop_spectating(conn);
        if let Some(binding) = self.bindings.remove(&conn.id) {
            binding.instance.leave(binding.player_id);
        }
        if conn.is_queue_mode {
            self.remove_preview_watcher(&conn.mode_id, conn.id);
            self.dequeue(&conn.mode_id, conn.id);
        } else if let Some(instance) = self.public_instance(&conn.mode_id) {
            instance.remove_viewer(conn.id);
        }
        self.cleanup_private_games();
    }

    /// Joins the public world, or the queue of a queue mode.
    fn join(&mut self, conn: &Connection, form: JoinForm) {
        let name = match self.validate_name(form.name) {
            Ok(name) => name,
            Err(error) => return conn.send_error(error),
        };
        self.stop_spectating(conn);
        self.detach_binding(conn.id);
        self.cleanup_private_games();
        self.dequeue(&conn.mode_id, conn.id);
        if conn.is_queue_mode {
            return self.enqueue(conn, name, form.kit_name);
        }

        let Some(instance) = self.public_instance(&conn.mode_id).cloned() else {
            return;
        };
        if form
            .player_id
            .is_some_and(|player_id| instance.has_active_session(player_id))
        {
            return conn.send_error(GameError::Custom {
                title: "Duplicate Session".to_string(),
                message: "You are already playing in another tab".to_string(),
            });
        }

        instance.remove_viewer(conn.id);
        let joined = instance.join(JoinRequest {
            name,
            kit_name: form.kit_name,
            tx: conn.tx.clone(),
            player_id: form.player_id,
            session_secret: form.session_secret,
        });
        match joined {
            Ok((player_id, secret)) => {
                self.bind(conn.id, player_id, instance.clone());
                self.send_init(&conn.tx, &instance, player_id, secret);
            }
            Err(error) => {
                conn.send_error(error);
                instance.add_viewer(conn.id, conn.tx.clone());
            }
        }
    }

    /// Leaves the current match, queue, or spectated board and goes back to watching.
    fn leave(&mut self, conn: &Connection) {
        self.stop_spectating(conn);
        if let Some(binding) = self.bindings.remove(&conn.id) {
            binding.instance.leave(binding.player_id);
            self.cleanup_private_games();
        } else {
            self.dequeue(&conn.mode_id, conn.id);
        }

        if conn.is_queue_mode {
            self.ensure_preview_watcher(conn);
        } else {
            self.watch_public(conn);
        }
    }

    /// Posts a chat message to the room the connection is looking at.
    fn chat(&self, conn: &Connection, name_hint: String, message: String) {
        let message: String = message
            .trim()
            .chars()
            .take(self.config.global.chat_max_chars())
            .collect();
        if message.is_empty() {
            return;
        }
        let Some(room) = self.chat_room_of(conn) else {
            return;
        };
        let (sender_name, sender_color) = self.chat_identity(conn, name_hint);
        room.post_chat(ChatLine {
            sender_name,
            sender_color,
            message,
            is_system: false,
            sent_at: now_ms(),
        });
    }

    /// Returns the instance whose chat room the connection is in.
    fn chat_room_of(&self, conn: &Connection) -> Option<Arc<GameInstance>> {
        if let Some(binding) = self.bindings.get(&conn.id) {
            return Some(binding.instance.clone());
        }
        if let Some(instance) = self.spectated_instance(conn) {
            return Some(instance.clone());
        }
        let watched = conn
            .is_queue_mode
            .then(|| {
                self.instances_of(&conn.mode_id)
                    .find(|instance| instance.has_viewer(conn.id))
            })
            .flatten();
        watched
            .or_else(|| self.public_instance(&conn.mode_id))
            .cloned()
    }

    /// Returns the name and color a connection chats with.
    ///
    /// Players use their in-game identity, queued players their queue name, named spectators
    /// the name they spectate under, and other watchers the name their client suggests.
    fn chat_identity(&self, conn: &Connection, name_hint: String) -> (String, ColorHex) {
        if let Some(identity) = self
            .bindings
            .get(&conn.id)
            .and_then(|binding| binding.instance.player_identity(binding.player_id))
        {
            return identity;
        }
        let name = self
            .spectated_instance(conn)
            .and_then(|instance| instance.spectator_name(conn.id))
            .or_else(|| self.queued_name(&conn.mode_id, conn.id))
            .unwrap_or_else(|| {
                let trimmed = name_hint.trim();
                if trimmed.is_empty() {
                    "Guest".to_string()
                } else {
                    trimmed.chars().take(MAX_GUEST_NAME_CHARS).collect()
                }
            });
        (name, ColorHex::from(GUEST_CHAT_COLOR))
    }

    /// Trims a requested player name, generating one when it is empty.
    fn validate_name(&self, input: String) -> Result<String, GameError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(self.generate_name());
        }
        if trimmed.len() > MAX_NAME_LEN {
            return Err(GameError::Custom {
                title: "Invalid Name".to_string(),
                message: "Name must be 32 characters or less".to_string(),
            });
        }
        Ok(trimmed.to_string())
    }

    /// Generates an "Adjective Noun" name from the configured name pool.
    pub(super) fn generate_name(&self) -> String {
        let pool = &self.config.name_pool;
        let mut rng = rand::thread_rng();
        let adjective = pool
            .adjectives
            .choose(&mut rng)
            .map_or("Unnamed", String::as_str);
        let mut noun = pool.nouns.choose(&mut rng).map_or("Player", String::as_str);
        if noun == adjective
            && let Some(another) = pool.nouns.choose(&mut rng)
        {
            noun = another;
        }
        format!("{adjective} {noun}")
    }
}
