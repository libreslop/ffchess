use self::session::SocketSession;
use self::throttle::MessageThrottle;
use crate::state::ServerState;
use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use common::protocol::ServerMessage;
use common::types::ModeId;
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use std::sync::Arc;
use tokio::sync::mpsc;

mod session;
mod throttle;

const SOCKET_BUFFER_CAPACITY: usize = 100;

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
        tracing::error!(%mode_id, "Game mode not found");
        return;
    };

    let is_queue_mode = state.queue_target_players(&mode_id).is_some();
    let (sender, mut receiver) = socket.split();
    let (tx, rx) = mpsc::channel::<ServerMessage>(SOCKET_BUFFER_CAPACITY);
    let send_task = spawn_socket_sender(sender, rx);

    let session = SocketSession::new(conn_id(), mode_id, state, lobby_instance, tx, is_queue_mode);
    session.register_connection().await;

    let mut throttle = MessageThrottle::new();
    while let Some(Ok(msg)) = receiver.next().await {
        if !throttle.allow_next() {
            continue;
        }
        if let Message::Text(text) = msg {
            session.handle_text_message(text).await;
        }
    }

    session.cleanup_connection().await;
    send_task.abort();
}

fn conn_id() -> crate::types::ConnectionId {
    crate::types::ConnectionId::new()
}

fn spawn_socket_sender(
    mut sender: SplitSink<WebSocket, Message>,
    mut rx: mpsc::Receiver<ServerMessage>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json = serde_json::to_string(&msg).unwrap_or_default();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    })
}
