use crate::state::ServerState;
use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use common::*;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
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
    state
        .player_channels
        .write()
        .await
        .insert(conn_id, tx.clone());

    // Send initial state immediately for background viewing
    {
        let game = state.game.read().await;
        let _ = tx.send(ServerMessage::Init {
            player_id: Uuid::nil(), // Nil UUID means not joined yet
            session_secret: Uuid::nil(),
            state: game.clone(),
        });
    }

    let mut player_id = None;

    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(client_msg) => {
                    match client_msg {
                        ClientMessage::Join {
                            name,
                            kit,
                            player_id: pid,
                            session_secret: secret,
                        } => {
                            tracing::info!(?name, ?kit, ?pid, "Player joining");

                            if let Some(pid) = pid {
                                let channels = state.player_channels.read().await;
                                let game = state.game.read().await;
                                if game.players.contains_key(&pid) && channels.contains_key(&pid) {
                                    tracing::warn!(?pid, "Player already in game, rejecting join");
                                    let _ = tx.send(ServerMessage::Error(GameError::Custom {
                                        title: "Duplicate Session".to_string(),
                                        message: "You are already playing in another tab".to_string(),
                                    }));
                                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                    break; 
                                }
                            }

                            // If this connection already has a player_id (re-joining without disconnect)
                            // Remove the old player before adding the new one to prevent leaks.
                            if let Some(old_pid) = player_id {
                                state.remove_player(old_pid).await;
                                player_id = None;
                            }

                            // Remove anonymous channel
                            state.player_channels.write().await.remove(&conn_id);

                            match state.add_player(name, kit, tx.clone(), pid, secret).await {
                                Ok((id, secret)) => {
                                    player_id = Some(id);
                                    // Re-send Init with proper player_id and session_secret
                                    let game = state.game.read().await;
                                    let _ = tx.send(ServerMessage::Init {
                                        player_id: id,
                                        session_secret: secret,
                                        state: game.clone(),
                                    });
                                }
                                Err(e) => {
                                    tracing::warn!(?pid, error = %e, "Join failed");
                                    let _ = tx.send(ServerMessage::Error(e));
                                    // Restore anonymous channel so they can still watch
                                    state.player_channels.write().await.insert(conn_id, tx.clone());
                                }
                            }
                        }
                        ClientMessage::MovePiece { piece_id, target } => {
                            if let Some(pid) = player_id
                                && let Err(e) = state.handle_move(pid, piece_id, target).await
                            {
                                tracing::warn!(?pid, ?piece_id, ?target, error = %e, "Invalid move");
                                let _ = tx.send(ServerMessage::Error(e));
                            }
                        }
                        ClientMessage::BuyPiece {
                            shop_pos,
                            piece_type,
                        } => {
                            if let Some(pid) = player_id
                                && let Err(e) =
                                    state.handle_shop_buy(pid, shop_pos, piece_type).await
                            {
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
