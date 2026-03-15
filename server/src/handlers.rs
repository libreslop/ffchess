//! Axum handlers for HTTP endpoints and WebSocket sessions.

use crate::state::ServerState;
use crate::types::ConnectionId;
use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use common::models::ModeSummary;
use common::protocol::{ClientMessage, GameError, ServerMessage};
use common::types::{ModeId, PlayerId, SessionSecret};
use futures_util::{SinkExt, StreamExt};
use jsonc_parser::parse_to_serde_value;
use rand::seq::SliceRandom;
use std::{fs, sync::Arc};
use tokio::sync::mpsc;

/// Builds a snapshot of current mode status for list endpoints.
///
/// `state` is the shared server state. Returns a vector of `ModeSummary`.
async fn mode_list_snapshot(state: &Arc<ServerState>) -> Vec<ModeSummary> {
    let mut list = Vec::new();
    let games = state.games.read().await;
    for (mode_id, instance) in games.iter() {
        let players = instance.game.read().await.players.len() as u32;
        list.push(ModeSummary {
            id: mode_id.clone(),
            display_name: instance.mode_config.display_name.clone(),
            players,
            max_players: instance.mode_config.max_players,
            respawn_cooldown_ms: instance.mode_config.respawn_cooldown_ms,
        });
    }
    list
}

/// Serves the index HTML with injected mode/global JSON.
///
/// `state` is extracted from Axum. Returns an HTML response.
pub async fn index_html(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let html_path = crate::paths::client_dist_dir().join("index.html");
    let html = fs::read_to_string(html_path)
        .unwrap_or_else(|_| "<!doctype html><body>missing index</body>".to_string());
    let modes_json = serde_json::to_string(&mode_list_snapshot(&state).await)
        .unwrap_or_else(|_| "[]".to_string());
    let global_json = fs::read_to_string("config/global/client.jsonc")
        .ok()
        .and_then(|raw| {
            parse_to_serde_value(&raw, &Default::default())
                .ok()
                .flatten()
                .and_then(|v| serde_json::to_string(&v).ok())
        })
        .unwrap_or_else(|| "{}".to_string());
    let replaced = html
        .replace("__MODES_JSON__", &modes_json)
        .replace("__GLOBAL_JSON__", &global_json);
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html")],
        replaced,
    )
}

/// Upgrades an HTTP connection to a game WebSocket session.
///
/// `mode_id` selects the game mode, `state` provides server state.
/// Returns an Axum response that completes the upgrade.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(mode_id): Path<String>,
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, ModeId::from(mode_id), state))
}

/// Lists current game modes and player counts.
///
/// `state` is extracted from Axum. Returns a JSON response.
pub async fn list_modes(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    axum::Json(mode_list_snapshot(&state).await)
}

/// Generates a human-friendly fallback player name.
///
/// `state` provides the name pool. Returns a generated display name.
fn generate_name(state: &ServerState) -> String {
    let pool = &state.config_manager.name_pool;
    let mut rng = rand::thread_rng();
    let adj = pool
        .adjectives
        .choose(&mut rng)
        .cloned()
        .unwrap_or_else(|| "Unnamed".to_string());
    let mut noun = pool
        .nouns
        .choose(&mut rng)
        .cloned()
        .unwrap_or_else(|| "Player".to_string());
    if noun == adj
        && let Some(n) = pool.nouns.choose(&mut rng)
    {
        noun = n.clone();
    }
    format!("{adj} {noun}")
}

/// Handles the lifecycle of a single WebSocket client session.
///
/// `socket` is the upgraded WebSocket, `mode_id` selects the mode, and `state` is shared.
/// Returns nothing; this runs until the socket closes.
async fn handle_socket(socket: WebSocket, mode_id: ModeId, state: Arc<ServerState>) {
    let instance = match state.get_game(&mode_id).await {
        Some(i) => i,
        None => {
            tracing::error!("Game mode not found: {}", mode_id);
            return;
        }
    };

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

    let conn_id = ConnectionId::new();
    instance
        .connection_channels
        .write()
        .await
        .insert(conn_id, tx.clone());

    {
        let game = instance.game.read().await;
        let _ = tx.send(ServerMessage::Init {
            player_id: PlayerId::nil(),
            session_secret: SessionSecret::nil(),
            state: Box::new(game.clone()),
            mode: instance.mode_config.to_client_config(),
            pieces: (*instance.piece_configs).clone(),
            shops: (*instance.shop_configs).clone(),
        });
    }

    let mut player_id: Option<PlayerId> = None;

    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(client_msg) => match client_msg {
                    ClientMessage::Join {
                        mut name,
                        kit_name,
                        player_id: pid,
                        session_secret: secret,
                    } => {
                        name = name.trim().to_string();
                        if name.is_empty() {
                            name = generate_name(&state);
                        }
                        if let Some(pid) = pid {
                            let channels = instance.player_channels.read().await;
                            let game = instance.game.read().await;
                            if game.players.contains_key(&pid) && channels.contains_key(&pid) {
                                let _ = tx.send(ServerMessage::Error(GameError::Custom {
                                    title: "Duplicate Session".to_string(),
                                    message: "You are already playing in another tab".to_string(),
                                }));
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                break;
                            }
                        }

                        if let Some(old_pid) = player_id {
                            instance.remove_player(old_pid).await;
                            player_id = None;
                        }

                        instance.connection_channels.write().await.remove(&conn_id);

                        match instance
                            .add_player(name, kit_name, tx.clone(), pid, secret)
                            .await
                        {
                            Ok((id, secret)) => {
                                player_id = Some(id);
                                let game = instance.game.read().await;
                                let _ = tx.send(ServerMessage::Init {
                                    player_id: id,
                                    session_secret: secret,
                                    state: Box::new(game.clone()),
                                    mode: instance.mode_config.to_client_config(),
                                    pieces: (*instance.piece_configs).clone(),
                                    shops: (*instance.shop_configs).clone(),
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(ServerMessage::Error(e));
                                instance
                                    .connection_channels
                                    .write()
                                    .await
                                    .insert(conn_id, tx.clone());
                            }
                        }
                    }
                    ClientMessage::MovePiece { piece_id, target } => {
                        if let Some(pid) = player_id
                            && let Err(e) = instance.handle_move(pid, piece_id, target).await
                        {
                            let _ = tx.send(ServerMessage::Error(e));
                        }
                    }
                    ClientMessage::BuyPiece {
                        shop_pos,
                        item_index,
                    } => {
                        if let Some(pid) = player_id
                            && let Err(e) =
                                instance.handle_shop_buy(pid, shop_pos, item_index).await
                        {
                            let _ = tx.send(ServerMessage::Error(e));
                        }
                    }
                    ClientMessage::Ping(t) => {
                        let _ = tx.send(ServerMessage::Pong(t));
                    }
                },
                Err(e) => {
                    tracing::error!(?text, error = %e, "Failed to parse client message");
                }
            }
        }
    }

    if let Some(pid) = player_id {
        instance.remove_player(pid).await;
    } else {
        instance.connection_channels.write().await.remove(&conn_id);
    }
    send_task.abort();
}
