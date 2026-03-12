use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use common::*;
use crate::state::ServerState;
use std::sync::Arc;
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use uuid::Uuid;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<ServerState>) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json = serde_json::to_string(&msg).unwrap();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    // Add to channels immediately so they get updates even before joining
    let conn_id = Uuid::new_v4();
    state.player_channels.write().await.insert(conn_id, tx.clone());

    // Send initial state immediately for background viewing
    {
        let game = state.game.read().await;
        let _ = tx.send(ServerMessage::Init { 
            player_id: Uuid::nil(), // Nil UUID means not joined yet
            state: game.clone() 
        });
    }

    let mut player_id = None;

    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(client_msg) => {
                    match client_msg {
                        ClientMessage::Join { name, kit, player_id: pid } => {
                            tracing::info!(?name, ?kit, ?pid, "Player joining");
                            // Remove anonymous channel
                            state.player_channels.write().await.remove(&conn_id);
                            
                            let id = state.add_player(name, kit, tx.clone(), pid).await;
                            player_id = Some(id);
                            
                            // Re-send Init with proper player_id
                            let game = state.game.read().await;
                            let _ = tx.send(ServerMessage::Init { 
                                player_id: id, 
                                state: game.clone() 
                            });
                        }
                        ClientMessage::MovePiece { piece_id, target } => {
                            if let Some(pid) = player_id
                                && let Err(e) = state.handle_move(pid, piece_id, target).await {
                                tracing::warn!(?pid, ?piece_id, ?target, error = %e, "Invalid move");
                                let _ = tx.send(ServerMessage::Error(e));
                            }
                        }
                        ClientMessage::BuyPiece { shop_pos, piece_type } => {
                            if let Some(pid) = player_id
                                && let Err(e) = state.handle_shop_buy(pid, shop_pos, piece_type).await {
                                tracing::warn!(?pid, ?shop_pos, ?piece_type, error = %e, "Shop buy failed");
                                let _ = tx.send(ServerMessage::Error(e));
                            }
                        }
                        ClientMessage::Ping(t) => {
                            let _ = tx.send(ServerMessage::Pong(t));
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(?text, error = %e, "Failed to parse client message");
                }
            }
        }
    }

    if let Some(pid) = player_id {
        tracing::info!(?pid, "Player leaving");
        state.remove_player(pid).await;
        state.player_channels.write().await.remove(&pid);
    } else {
        state.player_channels.write().await.remove(&conn_id);
    }
    send_task.abort();
}
