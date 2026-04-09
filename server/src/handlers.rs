//! Axum handlers for HTTP endpoints and WebSocket sessions.

use crate::state::{MatchQueueEntry, QueueJoinResult, ServerState};
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
use common::types::{ModeId, PlayerCount, PlayerId, QueuePosition, SessionSecret};
use futures_util::{SinkExt, StreamExt};
use jsonc_parser::parse_to_serde_value;
use rand::seq::SliceRandom;
use std::{fs, sync::Arc};
use tokio::sync::mpsc;

/// Builds a snapshot of current mode status for list endpoints.
///
/// `state` is the shared server state. Returns a vector of `ModeSummary`.
async fn mode_list_snapshot(state: &Arc<ServerState>) -> Vec<ModeSummary> {
    let public_games = state.public_game_instances().await;
    let private_games = state.private_game_instances().await;

    let mut list = Vec::new();
    for (mode_id, instance) in public_games {
        let queue_target = state.queue_target_players(&mode_id);
        let players = if queue_target.is_some() {
            let private_mode_prefix = format!("{}__", mode_id.as_ref());
            let mut active_match_players = PlayerCount::zero();
            for (private_id, private_instance) in &private_games {
                if private_id.as_ref().starts_with(&private_mode_prefix) {
                    active_match_players += private_instance.player_count().await;
                }
            }
            state.queue_len(&mode_id).await + active_match_players
        } else {
            instance.player_count().await
        };
        list.push(ModeSummary {
            id: mode_id.clone(),
            display_name: instance.mode_display_name().to_string(),
            players,
            max_players: queue_target.unwrap_or_else(|| instance.max_players()),
            queue_players: queue_target.unwrap_or_else(PlayerCount::zero),
            respawn_cooldown_ms: instance.respawn_cooldown_ms(),
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
    let pool = state.name_pool();
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

/// Broadcasts queue position/state updates to all queued players for a mode.
async fn broadcast_queue_state(state: &Arc<ServerState>, mode_id: &ModeId) {
    let Some((required_players, entries)) = state.queue_snapshot(mode_id).await else {
        return;
    };
    let queued_players = PlayerCount::new(entries.len() as u32);
    for (idx, entry) in entries.iter().enumerate() {
        let _ = entry.tx().send(ServerMessage::QueueState {
            position_in_queue: QueuePosition::new((idx + 1) as u32),
            queued_players,
            required_players,
        });
    }
}

/// Handles the lifecycle of a single WebSocket client session.
///
/// `socket` is the upgraded WebSocket, `mode_id` selects the mode, and `state` is shared.
/// Returns nothing; this runs until the socket closes.
async fn handle_socket(socket: WebSocket, mode_id: ModeId, state: Arc<ServerState>) {
    let lobby_instance = match state.get_joinable_game(&mode_id).await {
        Some(i) => i,
        None => {
            tracing::error!("Game mode not found: {}", mode_id);
            return;
        }
    };
    let is_queue_mode = state.queue_target_players(&mode_id).is_some();

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
    if is_queue_mode {
        state
            .ensure_preview_connection(&mode_id, conn_id, tx.clone())
            .await;
    } else {
        lobby_instance
            .add_connection_channel(conn_id, tx.clone())
            .await;
        state
            .send_init(&tx, &lobby_instance, PlayerId::nil(), SessionSecret::nil())
            .await;
    }

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

                        if let Some(binding) = state.unbind_connection(conn_id).await {
                            let (player_id, instance) = binding.into_parts();
                            instance.remove_player(player_id).await;
                            state.cleanup_private_games().await;
                        }

                        if state.remove_from_queue(&mode_id, conn_id).await {
                            broadcast_queue_state(&state, &mode_id).await;
                        }

                        if is_queue_mode {
                            state
                                .ensure_preview_connection(&mode_id, conn_id, tx.clone())
                                .await;
                            let queue_entry =
                                MatchQueueEntry::new(conn_id, tx.clone(), name, kit_name);
                            match state.enqueue_matchmaking(&mode_id, queue_entry).await {
                                Some(QueueJoinResult::Waiting) => {
                                    broadcast_queue_state(&state, &mode_id).await;
                                }
                                Some(QueueJoinResult::Matched {
                                    match_instance,
                                    players,
                                }) => {
                                    for qp in players {
                                        let (conn_id, tx, name, kit_name) = qp.into_parts();
                                        state.remove_preview_connection(&mode_id, conn_id).await;
                                        match match_instance
                                            .add_player(name, kit_name, tx.clone(), None, None)
                                            .await
                                        {
                                            Ok((id, session_secret)) => {
                                                state
                                                    .bind_connection(
                                                        conn_id,
                                                        id,
                                                        match_instance.clone(),
                                                    )
                                                    .await;
                                                state
                                                    .send_init(
                                                        &tx,
                                                        &match_instance,
                                                        id,
                                                        session_secret,
                                                    )
                                                    .await;
                                            }
                                            Err(e) => {
                                                let _ = tx.send(ServerMessage::Error(e));
                                            }
                                        }
                                    }
                                    state.refresh_preview_for_mode(&mode_id).await;
                                    broadcast_queue_state(&state, &mode_id).await;
                                }
                                None => {
                                    let _ = tx.send(ServerMessage::Error(GameError::Internal(
                                        "Matchmaking mode is not configured".to_string(),
                                    )));
                                }
                            }
                            continue;
                        }

                        if let Some(pid) = pid
                            && lobby_instance.has_active_player_session(pid).await
                        {
                            let _ = tx.send(ServerMessage::Error(GameError::Custom {
                                title: "Duplicate Session".to_string(),
                                message: "You are already playing in another tab".to_string(),
                            }));
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            break;
                        }

                        lobby_instance.remove_connection_channel(conn_id).await;

                        match lobby_instance
                            .add_player(name, kit_name, tx.clone(), pid, secret)
                            .await
                        {
                            Ok((id, session_secret)) => {
                                state
                                    .bind_connection(conn_id, id, lobby_instance.clone())
                                    .await;
                                state
                                    .send_init(&tx, &lobby_instance, id, session_secret)
                                    .await;
                            }
                            Err(e) => {
                                let _ = tx.send(ServerMessage::Error(e));
                                lobby_instance
                                    .add_connection_channel(conn_id, tx.clone())
                                    .await;
                            }
                        }
                    }
                    ClientMessage::MovePiece { piece_id, target } => {
                        if let Some(binding) = state.get_binding(conn_id).await
                            && let Err(e) = binding
                                .instance()
                                .handle_move(binding.player_id(), piece_id, target)
                                .await
                        {
                            let _ = tx.send(ServerMessage::Error(e));
                        }
                    }
                    ClientMessage::BuyPiece {
                        shop_pos,
                        item_index,
                    } => {
                        if let Some(binding) = state.get_binding(conn_id).await
                            && let Err(e) = binding
                                .instance()
                                .handle_shop_buy(binding.player_id(), shop_pos, item_index)
                                .await
                        {
                            let _ = tx.send(ServerMessage::Error(e));
                        }
                    }
                    ClientMessage::SetPreviewDefault { enabled } => {
                        if is_queue_mode {
                            state
                                .set_preview_default(&mode_id, conn_id, tx.clone(), enabled)
                                .await;
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

    if let Some(binding) = state.unbind_connection(conn_id).await {
        let (player_id, instance) = binding.into_parts();
        instance.remove_player(player_id).await;
        state.cleanup_private_games().await;
    } else {
        if is_queue_mode {
            state.remove_preview_connection(&mode_id, conn_id).await;
        } else {
            lobby_instance.remove_connection_channel(conn_id).await;
        }
        if state.remove_from_queue(&mode_id, conn_id).await {
            broadcast_queue_state(&state, &mode_id).await;
        }
    }
    send_task.abort();
}
