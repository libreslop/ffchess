//! WebSocket endpoint connecting clients to the lobby.

mod throttle;

use crate::state::ServerState;
use crate::time::now_ms;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use common::protocol::{ClientMessage, ServerMessage};
use common::types::ModeId;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use throttle::MessageThrottle;
use tokio::sync::mpsc;

/// Outbound messages buffered per connection before new ones are dropped.
const SOCKET_BUFFER_CAPACITY: usize = 256;

/// Upgrades an HTTP connection to a game WebSocket session.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(mode_id): Path<String>,
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, ModeId::from(mode_id), state))
}

/// Runs one WebSocket session until the client disconnects.
async fn handle_socket(socket: WebSocket, mode_id: ModeId, state: Arc<ServerState>) {
    let (sender, mut receiver) = socket.split();
    let (tx, rx) = mpsc::channel::<ServerMessage>(SOCKET_BUFFER_CAPACITY);
    let Some(conn) = state.open_connection(mode_id.clone(), tx) else {
        tracing::error!(%mode_id, "Game mode not found");
        return;
    };
    let send_task = spawn_socket_sender(sender, rx);

    let mut throttle = MessageThrottle::new(now_ms());
    while let Some(Ok(message)) = receiver.next().await {
        let Message::Text(text) = message else {
            continue;
        };
        if !throttle.allow(now_ms()) {
            tracing::warn!(%mode_id, "Dropping message from a client over the rate limit");
            continue;
        }
        match serde_json::from_str::<ClientMessage>(&text) {
            Ok(client_message) => state.handle_message(&conn, client_message),
            Err(error) => tracing::error!(?text, %error, "Failed to parse client message"),
        }
    }

    state.close_connection(&conn);
    send_task.abort();
}

/// Forwards queued server messages to the socket as JSON text frames.
fn spawn_socket_sender(
    mut sender: SplitSink<WebSocket, Message>,
    mut rx: mpsc::Receiver<ServerMessage>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let Ok(json) = serde_json::to_string(&message) else {
                continue;
            };
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    })
}
