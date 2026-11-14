//! WebSocket connection loop for the client app.

use crate::reducer::{
    DisconnectNotice, GameAction, GameStateReducer, InitPayload, QueueStatus, UpdateStatePayload,
};
use crate::utils::{SessionStore, local_now};
use common::protocol::{GameError, ServerMessage};
use common::types::ModeId;
use futures_util::{SinkExt, StreamExt};
use gloo_net::websocket::{Message, futures::WebSocket};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::UseReducerHandle;

/// Delay before reconnecting after the socket closed or failed to open.
const RECONNECT_DELAY_MS: u32 = 1500;

/// Outgoing websocket frames buffered while the socket is busy.
const OUTGOING_BUFFER: usize = 256;

/// Shared handle to the latest reducer handle.
pub type ReducerRef = Rc<RefCell<UseReducerHandle<GameStateReducer>>>;

/// Shared slot holding the sender of the currently open socket.
pub type SocketSenderRef = Rc<RefCell<Option<mpsc::Sender<Message>>>>;

/// Connects to the server WebSocket and dispatches incoming messages.
///
/// Reconnects forever; the caller aborts the future to stop.
pub async fn connect_ws(
    ws_url: String,
    mode_id: ModeId,
    reducer: ReducerRef,
    socket_tx: SocketSenderRef,
) {
    loop {
        match WebSocket::open(&ws_url) {
            Ok(ws) => {
                let (mut write, mut read) = ws.split();
                let (tx, mut rx) = mpsc::channel::<Message>(OUTGOING_BUFFER);
                *socket_tx.borrow_mut() = Some(tx);
                spawn_local(async move {
                    while let Some(message) = rx.recv().await {
                        if let Err(error) = write.send(message).await {
                            web_sys::console::error_1(
                                &format!("WebSocket send failed: {error}").into(),
                            );
                        }
                    }
                });

                let mut is_first_message = true;
                while let Some(message) = read.next().await {
                    match message {
                        Ok(Message::Text(text)) => {
                            match serde_json::from_str::<ServerMessage>(&text) {
                                Ok(server_message) => {
                                    if std::mem::take(&mut is_first_message) {
                                        reducer
                                            .borrow()
                                            .dispatch(GameAction::ConnectionEstablished);
                                    }
                                    let action = to_action(server_message, &mode_id);
                                    reducer.borrow().dispatch(action);
                                }
                                Err(_) => web_sys::console::error_1(
                                    &format!("Failed to decode server message: {text}").into(),
                                ),
                            }
                        }
                        Ok(_) => {}
                        Err(error) => web_sys::console::error_1(
                            &format!("WebSocket receive failed: {error}").into(),
                        ),
                    }
                }
                *socket_tx.borrow_mut() = None;
            }
            Err(_) => web_sys::console::error_1(&"WebSocket open failed; retrying.".into()),
        }

        report_connection_lost(&reducer);
        gloo_timers::future::TimeoutFuture::new(RECONNECT_DELAY_MS).await;
    }
}

/// Shows the disconnected overlay unless another overlay already explains the situation.
pub fn report_connection_lost(reducer: &ReducerRef) {
    let current = reducer.borrow().clone();
    if !current.disconnected && !current.fatal_error {
        current.dispatch(GameAction::SetDisconnected(
            DisconnectNotice::connection_lost(),
        ));
    }
}

/// Converts a server message into a reducer action, persisting session data on the way.
fn to_action(message: ServerMessage, mode_id: &ModeId) -> GameAction {
    match message {
        ServerMessage::Init {
            player_id,
            session_secret,
            move_unlock_at,
            state,
            mode,
            catalog,
            chat_room_key,
            chat_history,
            spectators,
            sync_interval_ms: _,
        } => {
            if !player_id.is_nil() {
                SessionStore::new(mode_id).save(player_id, session_secret);
            }
            GameAction::SetInit(Box::new(InitPayload {
                player_id,
                session_secret,
                move_unlock_at,
                state: *state,
                mode: *mode,
                catalog: *catalog,
                chat_room_key,
                chat_history,
                spectators,
            }))
        }
        ServerMessage::UpdateState {
            players,
            pieces,
            shops,
            board_size,
            spectators,
        } => GameAction::UpdateState(Box::new(UpdateStatePayload {
            players,
            pieces,
            shops,
            board_size,
            spectators,
        })),
        ServerMessage::ActionResult {
            id,
            result,
            culprits,
        } => GameAction::ActionResult {
            id,
            result,
            culprits,
        },
        ServerMessage::QueueState {
            position_in_queue,
            queued_players,
            required_players,
        } => GameAction::SetQueueStatus(QueueStatus {
            position_in_queue,
            queued_players,
            required_players,
            since: local_now(),
        }),
        ServerMessage::Error(GameError::Custom { title, message }) => {
            GameAction::SetDisconnected(DisconnectNotice::fatal(title, message))
        }
        ServerMessage::Error(error) => GameAction::SetError(error),
        ServerMessage::Victory {
            title,
            message,
            focus_target,
        } => GameAction::SetVictory {
            title,
            msg: message,
            focus_target,
        },
        ServerMessage::GameOver(stats) => GameAction::GameOver(stats),
        ServerMessage::Pong(sent_at, server_time) => GameAction::Pong(sent_at, server_time),
        ServerMessage::Chat { line } => GameAction::PushChatLine(line),
        ServerMessage::MatchList { matches } => GameAction::SetMatchList(matches),
        ServerMessage::SpectateEnded => GameAction::SpectateEnded,
    }
}
