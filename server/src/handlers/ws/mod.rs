//! WebSocket endpoint connecting clients to the lobby.

mod origin;
mod throttle;

use crate::outbound::{ClientFeed, ClientTx};
use crate::state::ServerState;
use crate::time::now_ms;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use common::protocol::ClientMessage;
use common::types::ModeId;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use origin::UpgradeOrigin;
use std::sync::Arc;
use std::time::Duration;
use throttle::MessageThrottle;

/// Upgrades an HTTP connection to a game WebSocket session.
///
/// Browsers may only connect from pages of this server or of a configured origin.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(mode_id): Path<String>,
    headers: HeaderMap,
    State(state): State<Arc<ServerState>>,
) -> Response {
    if !UpgradeOrigin::of(&headers).is_allowed(&state.config().global.allowed_origins) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.on_upgrade(|socket| handle_socket(socket, ModeId::from(mode_id), state))
}

/// Runs one WebSocket session until the client disconnects or stops keeping up.
async fn handle_socket(socket: WebSocket, mode_id: ModeId, state: Arc<ServerState>) {
    let (sender, mut receiver) = socket.split();
    let global = &state.config().global;
    let (tx, feed) = ClientTx::channel(global.socket_event_capacity());
    let stall_timeout = global.socket_stall_timeout();
    let Some(conn) = state.open_connection(mode_id.clone(), tx) else {
        tracing::error!(%mode_id, "Game mode not found");
        return;
    };
    let mut send_task = tokio::spawn(forward_to_socket(sender, feed, stall_timeout));

    let mut throttle = MessageThrottle::new(now_ms());
    loop {
        let message = tokio::select! {
            message = receiver.next() => message,
            // The writer gave up on the client, so the session ends too.
            _ = &mut send_task => break,
        };
        let Some(Ok(message)) = message else {
            break;
        };
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

/// Writes a connection's outbound traffic to its socket as JSON text frames.
///
/// Stops when the connection must close, when the socket fails, or when one write stays
/// blocked for `stall_timeout`, so a client that never reads cannot hold the task forever.
async fn forward_to_socket(
    mut sender: SplitSink<WebSocket, Message>,
    mut feed: ClientFeed,
    stall_timeout: Duration,
) {
    while let Some(batch) = feed.next_batch().await {
        for text in batch {
            let written = tokio::time::timeout(stall_timeout, sender.send(Message::Text(text)));
            if !matches!(written.await, Ok(Ok(()))) {
                return;
            }
        }
    }
}
