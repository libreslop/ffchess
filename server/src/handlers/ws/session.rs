use crate::state::{MatchQueueEntry, QueueJoinResult, ServerState};
use crate::time::now_ms;
use crate::types::ConnectionId;
use common::protocol::{ChatLine, ClientMessage, GameError, ServerMessage};
use common::types::ColorHex;
use common::types::{BoardCoord, KitId, ModeId, PieceId, PlayerId, SessionSecret};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Stateful websocket session bound to one connection.
pub(super) struct SocketSession {
    conn_id: ConnectionId,
    mode_id: ModeId,
    state: Arc<ServerState>,
    lobby_instance: Arc<crate::instance::GameInstance>,
    tx: mpsc::Sender<ServerMessage>,
    is_queue_mode: bool,
}

impl SocketSession {
    /// Creates a new connection session wrapper.
    pub(super) fn new(
        conn_id: ConnectionId,
        mode_id: ModeId,
        state: Arc<ServerState>,
        lobby_instance: Arc<crate::instance::GameInstance>,
        tx: mpsc::Sender<ServerMessage>,
        is_queue_mode: bool,
    ) -> Self {
        Self {
            conn_id,
            mode_id,
            state,
            lobby_instance,
            tx,
            is_queue_mode,
        }
    }

    /// Registers this connection for preview/lobby updates.
    pub(super) async fn register_connection(&self) {
        if self.is_queue_mode {
            self.state
                .ensure_preview_connection(&self.mode_id, self.conn_id, self.tx.clone())
                .await;
            return;
        }

        self.lobby_instance
            .add_connection_channel(self.conn_id, self.tx.clone())
            .await;
        self.state
            .send_init(
                &self.tx,
                &self.lobby_instance,
                PlayerId::nil(),
                SessionSecret::nil(),
            )
            .await;
    }

    /// Parses and handles one text websocket payload.
    pub(super) async fn handle_text_message(&self, text: String) {
        match serde_json::from_str::<ClientMessage>(&text) {
            Ok(client_msg) => self.handle_client_message(client_msg).await,
            Err(error) => {
                tracing::error!(?text, error = %error, "Failed to parse client message");
            }
        }
    }

    async fn handle_client_message(&self, client_msg: ClientMessage) {
        match client_msg {
            ClientMessage::Join {
                name,
                kit_name,
                player_id,
                session_secret,
            } => {
                self.handle_join(name, kit_name, player_id, session_secret)
                    .await;
            }
            ClientMessage::Leave => self.handle_leave().await,
            ClientMessage::MovePiece { piece_id, target } => {
                self.handle_move(piece_id, target).await;
            }
            ClientMessage::BuyPiece {
                shop_pos,
                item_index,
            } => {
                self.handle_shop_buy(shop_pos, item_index).await;
            }
            ClientMessage::ClearPremoves { piece_id } => {
                if let Some(binding) = self.state.get_binding(self.conn_id).await {
                    binding.instance().clear_queued_moves(piece_id).await;
                }
            }
            ClientMessage::SetPreviewDefault { enabled } => {
                if self.is_queue_mode {
                    self.state
                        .set_preview_default(&self.mode_id, self.conn_id, self.tx.clone(), enabled)
                        .await;
                }
            }
            ClientMessage::Chat { name_hint, message } => {
                self.handle_chat(name_hint, message).await;
            }
            ClientMessage::Ping(client_now) => {
                let _ = self
                    .tx
                    .try_send(ServerMessage::Pong(client_now, crate::time::now_ms()));
            }
        }
    }

    async fn handle_move(&self, piece_id: PieceId, target: BoardCoord) {
        if let Some(binding) = self.state.get_binding(self.conn_id).await
            && let Err(error) = binding
                .instance()
                .handle_move(binding.player_id(), piece_id, target)
                .await
        {
            let _ = self.tx.try_send(ServerMessage::Error(error));
        }
    }

    async fn handle_shop_buy(&self, shop_pos: BoardCoord, item_index: usize) {
        if let Some(binding) = self.state.get_binding(self.conn_id).await
            && let Err(error) = binding
                .instance()
                .handle_shop_buy(binding.player_id(), shop_pos, item_index)
                .await
        {
            let _ = self.tx.try_send(ServerMessage::Error(error));
        }
    }

    async fn handle_join(
        &self,
        raw_name: String,
        kit_name: KitId,
        player_id: Option<PlayerId>,
        session_secret: Option<SessionSecret>,
    ) {
        let name = match super::super::name::ValidPlayerName::from_input(raw_name, &self.state) {
            Ok(name) => name.into_inner(),
            Err(error) => {
                let _ = self.tx.try_send(ServerMessage::Error(error));
                return;
            }
        };

        self.detach_existing_binding_for_rejoin().await;
        self.remove_from_queue_if_present().await;

        if self.is_queue_mode {
            self.handle_queue_join(name, kit_name).await;
            return;
        }

        if let Some(player_id) = player_id
            && self
                .lobby_instance
                .has_active_player_session(player_id)
                .await
        {
            let _ = self.tx.try_send(ServerMessage::Error(GameError::Custom {
                title: "Duplicate Session".to_string(),
                message: "You are already playing in another tab".to_string(),
            }));
            return;
        }

        self.lobby_instance
            .remove_connection_channel(self.conn_id)
            .await;

        match self
            .lobby_instance
            .add_player(name, kit_name, self.tx.clone(), player_id, session_secret)
            .await
        {
            Ok((id, secret)) => {
                self.state
                    .bind_connection(self.conn_id, id, self.lobby_instance.clone())
                    .await;
                self.state
                    .send_init(&self.tx, &self.lobby_instance, id, secret)
                    .await;
            }
            Err(error) => {
                let _ = self.tx.try_send(ServerMessage::Error(error));
                self.lobby_instance
                    .add_connection_channel(self.conn_id, self.tx.clone())
                    .await;
            }
        }
    }

    async fn handle_queue_join(&self, name: String, kit_name: KitId) {
        self.state
            .ensure_preview_connection(&self.mode_id, self.conn_id, self.tx.clone())
            .await;

        let queue_entry = MatchQueueEntry::new(self.conn_id, self.tx.clone(), name, kit_name);
        match self
            .state
            .enqueue_matchmaking(&self.mode_id, queue_entry)
            .await
        {
            Some(QueueJoinResult::Waiting) => {
                super::super::queue::broadcast_queue_state(&self.state, &self.mode_id).await;
            }
            Some(QueueJoinResult::Matched {
                match_instance,
                players,
            }) => {
                for queued_player in players {
                    let (conn_id, tx, name, kit_name) = queued_player.into_parts();
                    self.state
                        .remove_preview_connection(&self.mode_id, conn_id)
                        .await;
                    match match_instance
                        .add_player(name, kit_name, tx.clone(), None, None)
                        .await
                    {
                        Ok((id, secret)) => {
                            self.state
                                .bind_connection(conn_id, id, match_instance.clone())
                                .await;
                            self.state.send_init(&tx, &match_instance, id, secret).await;
                        }
                        Err(error) => {
                            let _ = tx.try_send(ServerMessage::Error(error));
                        }
                    }
                }
                self.state.refresh_preview_for_mode(&self.mode_id).await;
                super::super::queue::broadcast_queue_state(&self.state, &self.mode_id).await;
            }
            None => {
                let _ = self.tx.try_send(ServerMessage::Error(GameError::Internal(
                    "Matchmaking mode is not configured".to_string(),
                )));
            }
        }
    }

    async fn handle_leave(&self) {
        if let Some(binding) = self.state.unbind_connection(self.conn_id).await {
            let (player_id, instance) = binding.into_parts();
            instance.remove_player(player_id).await;
            self.state.cleanup_private_games().await;
        } else if self.is_queue_mode
            && self
                .state
                .remove_from_queue(&self.mode_id, self.conn_id)
                .await
        {
            super::super::queue::broadcast_queue_state(&self.state, &self.mode_id).await;
        }

        if self.is_queue_mode {
            self.state
                .ensure_preview_connection(&self.mode_id, self.conn_id, self.tx.clone())
                .await;
            return;
        }

        self.lobby_instance
            .add_connection_channel(self.conn_id, self.tx.clone())
            .await;
        self.state
            .send_init(
                &self.tx,
                &self.lobby_instance,
                PlayerId::nil(),
                SessionSecret::nil(),
            )
            .await;
    }

    async fn detach_existing_binding_for_rejoin(&self) {
        if let Some(binding) = self.state.unbind_connection(self.conn_id).await {
            let (player_id, instance) = binding.into_parts();
            instance.detach_player(player_id).await;
            self.state.cleanup_private_games().await;
        }
    }

    async fn remove_from_queue_if_present(&self) {
        if self
            .state
            .remove_from_queue(&self.mode_id, self.conn_id)
            .await
        {
            super::super::queue::broadcast_queue_state(&self.state, &self.mode_id).await;
        }
    }

    async fn handle_chat(&self, name_hint: String, message: String) {
        let trimmed = message.trim();
        if trimmed.is_empty() {
            return;
        }
        let message = trimmed
            .chars()
            .take(self.state.chat_message_max_chars())
            .collect::<String>();

        let target_instance = if let Some(binding) = self.state.get_binding(self.conn_id).await {
            binding.instance().clone()
        } else if self.is_queue_mode {
            if let Some(instance) = self
                .state
                .watched_instance_for_connection(&self.mode_id, self.conn_id)
                .await
            {
                instance
            } else if let Some(instance) = self.state.get_joinable_game(&self.mode_id).await {
                instance
            } else {
                self.lobby_instance.clone()
            }
        } else {
            self.lobby_instance.clone()
        };

        let (sender_name, sender_color) = self.chat_identity(&target_instance, name_hint).await;
        let line = ChatLine {
            sender_name,
            sender_color,
            message,
            is_system: false,
            sent_at: now_ms(),
        };
        target_instance.push_chat_line(line.clone()).await;
        target_instance
            .broadcast(ServerMessage::Chat { line })
            .await;
    }

    async fn chat_identity(
        &self,
        instance: &Arc<crate::instance::GameInstance>,
        name_hint: String,
    ) -> (String, ColorHex) {
        if let Some(binding) = self.state.get_binding(self.conn_id).await
            && let Some(player) = binding
                .instance()
                .game
                .read()
                .await
                .players
                .get(&binding.player_id())
        {
            return (player.name.clone(), player.color.clone());
        }

        if self.is_queue_mode
            && let Some(name) = self.state.queued_name(&self.mode_id, self.conn_id).await
        {
            return (name, ColorHex::from("#555555"));
        }

        let sanitized_name = sanitize_chat_name(name_hint);
        let _ = instance;
        (sanitized_name, ColorHex::from("#555555"))
    }

    /// Cleans up all bindings/channels associated with a disconnected socket.
    pub(super) async fn cleanup_connection(&self) {
        if let Some(binding) = self.state.unbind_connection(self.conn_id).await {
            let (player_id, instance) = binding.into_parts();
            let disconnected_name = instance
                .game
                .read()
                .await
                .players
                .get(&player_id)
                .map(|player| player.name.clone());
            if let Some(player_name) = disconnected_name {
                instance.record_player_disconnect_event(player_name).await;
            }
            instance.remove_player(player_id).await;
            self.state.cleanup_private_games().await;
            return;
        }

        if self.is_queue_mode {
            self.state
                .remove_preview_connection(&self.mode_id, self.conn_id)
                .await;
        } else {
            self.lobby_instance
                .remove_connection_channel(self.conn_id)
                .await;
        }

        if self
            .state
            .remove_from_queue(&self.mode_id, self.conn_id)
            .await
        {
            super::super::queue::broadcast_queue_state(&self.state, &self.mode_id).await;
        }
    }
}

fn sanitize_chat_name(input: String) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "Guest".to_string();
    }
    trimmed.chars().take(32).collect()
}
