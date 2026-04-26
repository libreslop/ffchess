use super::name::ValidPlayerName;
use super::queue::broadcast_queue_state;
use crate::state::{MatchQueueEntry, QueueJoinResult, ServerState};
use crate::types::ConnectionId;
use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use common::protocol::{ClientMessage, GameError, ServerMessage};
use common::types::{ModeId, PlayerId, SessionSecret};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Upgrades an HTTP connection to a game WebSocket session.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(mode_id): Path<String>,
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, ModeId::from(mode_id), state))
}

/// Handles the lifecycle of a single WebSocket client session.
async fn handle_socket(socket: WebSocket, mode_id: ModeId, state: Arc<ServerState>) {
    let Some(lobby_instance) = state.get_joinable_game(&mode_id).await else {
        tracing::error!("Game mode not found: {}", mode_id);
        return;
    };
    let is_queue_mode = state.queue_target_players(&mode_id).is_some();

    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<ServerMessage>(100);

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json = serde_json::to_string(&msg).unwrap_or_default();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    let conn_id = ConnectionId::new();
    let session = SocketSession {
        conn_id,
        mode_id,
        state,
        lobby_instance,
        tx,
        is_queue_mode,
    };

    session.register_connection().await;

    let mut last_message_at = crate::time::now_ms();
    while let Some(Ok(msg)) = receiver.next().await {
        if !can_accept_message(last_message_at) {
            continue;
        }
        last_message_at = crate::time::now_ms();

        if let Message::Text(text) = msg {
            session.handle_text_message(text).await;
        }
    }

    session.cleanup_connection().await;
    send_task.abort();
}

fn can_accept_message(last_message_at: common::types::TimestampMs) -> bool {
    let now = crate::time::now_ms();
    now - last_message_at >= common::types::DurationMs::from_millis(50)
}

struct SocketSession {
    conn_id: ConnectionId,
    mode_id: ModeId,
    state: Arc<ServerState>,
    lobby_instance: Arc<crate::instance::GameInstance>,
    tx: mpsc::Sender<ServerMessage>,
    is_queue_mode: bool,
}

impl SocketSession {
    async fn register_connection(&self) {
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

    async fn handle_text_message(&self, text: String) {
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
            ClientMessage::Leave => {
                self.handle_leave().await;
            }
            ClientMessage::MovePiece { piece_id, target } => {
                if let Some(binding) = self.state.get_binding(self.conn_id).await
                    && let Err(error) = binding
                        .instance()
                        .handle_move(binding.player_id(), piece_id, target)
                        .await
                {
                    let _ = self.tx.try_send(ServerMessage::Error(error));
                }
            }
            ClientMessage::BuyPiece {
                shop_pos,
                item_index,
            } => {
                if let Some(binding) = self.state.get_binding(self.conn_id).await
                    && let Err(error) = binding
                        .instance()
                        .handle_shop_buy(binding.player_id(), shop_pos, item_index)
                        .await
                {
                    let _ = self.tx.try_send(ServerMessage::Error(error));
                }
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
            ClientMessage::Ping(client_now) => {
                let _ = self
                    .tx
                    .try_send(ServerMessage::Pong(client_now, crate::time::now_ms()));
            }
        }
    }

    async fn handle_join(
        &self,
        raw_name: String,
        kit_name: common::types::KitId,
        player_id: Option<PlayerId>,
        session_secret: Option<SessionSecret>,
    ) {
        let name = match ValidPlayerName::from_input(raw_name, &self.state) {
            Ok(name) => name.into_inner(),
            Err(error) => {
                let _ = self.tx.try_send(ServerMessage::Error(error));
                return;
            }
        };

        self.detach_existing_binding().await;
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

    async fn handle_queue_join(&self, name: String, kit_name: common::types::KitId) {
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
                broadcast_queue_state(&self.state, &self.mode_id).await;
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
                broadcast_queue_state(&self.state, &self.mode_id).await;
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
            broadcast_queue_state(&self.state, &self.mode_id).await;
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

    async fn detach_existing_binding(&self) {
        if let Some(binding) = self.state.unbind_connection(self.conn_id).await {
            let (player_id, instance) = binding.into_parts();
            instance.remove_player(player_id).await;
            self.state.cleanup_private_games().await;
        }
    }

    async fn remove_from_queue_if_present(&self) {
        if self
            .state
            .remove_from_queue(&self.mode_id, self.conn_id)
            .await
        {
            broadcast_queue_state(&self.state, &self.mode_id).await;
        }
    }

    async fn cleanup_connection(&self) {
        if let Some(binding) = self.state.unbind_connection(self.conn_id).await {
            let (player_id, instance) = binding.into_parts();
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
            broadcast_queue_state(&self.state, &self.mode_id).await;
        }
    }
}
