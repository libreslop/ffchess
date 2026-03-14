use crate::state::ServerState;
use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use common::protocol::{ClientMessage, GameError, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use std::{fs, sync::Arc};
use tokio::sync::mpsc;
use uuid::Uuid;

fn to_client_mode(cfg: &common::models::GameModeConfig) -> common::models::GameModeClientConfig {
    common::models::GameModeClientConfig {
        id: cfg.id.clone(),
        display_name: cfg.display_name.clone(),
        camera_pan_limit: cfg.camera_pan_limit.clone(),
        fog_of_war_radius: cfg.fog_of_war_radius.clone(),
        kits: cfg
            .kits
            .iter()
            .map(|k| common::models::KitSummary {
                name: k.name.clone(),
                description: k.description.clone(),
                pieces: k.pieces.clone(),
            })
            .collect(),
    }
}

#[derive(Serialize)]
pub struct ModeSummary {
    pub id: String,
    pub display_name: String,
    pub players: u32,
    pub max_players: u32,
}

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
        });
    }
    list
}

pub async fn index_html(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let html = fs::read_to_string("client/dist/index.html")
        .unwrap_or_else(|_| "<!doctype html><body>missing index</body>".to_string());
    let modes_json = serde_json::to_string(&mode_list_snapshot(&state).await)
        .unwrap_or_else(|_| "[]".to_string());
    let replaced = html.replace("__MODES_JSON__", &modes_json);
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html")],
        replaced,
    )
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(mode_id): Path<String>,
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, mode_id, state))
}

pub async fn list_modes(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    axum::Json(mode_list_snapshot(&state).await)
}

async fn handle_socket(socket: WebSocket, mode_id: String, state: Arc<ServerState>) {
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

    let conn_id = Uuid::new_v4();
    instance
        .player_channels
        .write()
        .await
        .insert(conn_id, tx.clone());

    {
        let game = instance.game.read().await;
        let _ = tx.send(ServerMessage::Init {
            player_id: Uuid::nil(),
            session_secret: Uuid::nil(),
            state: game.clone(),
            mode: to_client_mode(&instance.mode_config),
            pieces: (*instance.piece_configs).clone(),
            shops: (*instance.shop_configs).clone(),
        });
    }

    let mut player_id = None;

    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(client_msg) => match client_msg {
                    ClientMessage::Join {
                        name,
                        kit_name,
                        player_id: pid,
                        session_secret: secret,
                    } => {
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

                        instance.player_channels.write().await.remove(&conn_id);

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
                                    state: game.clone(),
                                    mode: to_client_mode(&instance.mode_config),
                                    pieces: (*instance.piece_configs).clone(),
                                    shops: (*instance.shop_configs).clone(),
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(ServerMessage::Error(e));
                                instance
                                    .player_channels
                                    .write()
                                    .await
                                    .insert(conn_id, tx.clone());
                            }
                        }
                    }
                    ClientMessage::MovePiece { piece_id, target } => {
                        if let Some(pid) = player_id {
                            if let Err(e) = instance.handle_move(pid, piece_id, target).await {
                                let _ = tx.send(ServerMessage::Error(e));
                            }
                        }
                    }
                    ClientMessage::BuyPiece {
                        shop_pos,
                        item_index,
                    } => {
                        if let Some(pid) = player_id {
                            if let Err(e) =
                                instance.handle_shop_buy(pid, shop_pos, item_index).await
                            {
                                let _ = tx.send(ServerMessage::Error(e));
                            }
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
        instance.player_channels.write().await.remove(&pid);
    } else {
        instance.player_channels.write().await.remove(&conn_id);
    }
    send_task.abort();
}
